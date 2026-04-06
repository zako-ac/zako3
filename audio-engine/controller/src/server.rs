use std::sync::Arc;

use dashmap::DashMap;
use futures_util::StreamExt;
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    types::{AMQPValue, FieldTable, ShortString},
};
use tracing::{error, info, warn};

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_core::types::{ChannelId, GuildId};

use zako3_audio_engine_client::{AudioEngineRequest, AudioEngineResponse};

pub struct AudioEngineServer {
    pub session_manager: Arc<SessionManager>,
    pub rabbitmq_url: String,
    pub max_retries: u32,
    pub session_consumers: DashMap<GuildId, tokio::task::JoinHandle<()>>,
}

impl AudioEngineServer {
    pub fn new(
        session_manager: Arc<SessionManager>,
        rabbitmq_url: String,
        max_retries: u32,
    ) -> Self {
        Self {
            session_manager,
            rabbitmq_url,
            max_retries,
            session_consumers: DashMap::new(),
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), lapin::Error> {
        let conn = Arc::new(
            Connection::connect(&self.rabbitmq_url, ConnectionProperties::default()).await?,
        );
        let channel = conn.create_channel().await?;

        // Declare the shared control queue
        channel
            .queue_declare(
                "audio_engine_control".into(),
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        let mut consumer = channel
            .basic_consume(
                "audio_engine_control".into(),
                "audio_engine_control_consumer".into(),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        info!("Audio Engine listening for control messages on audio_engine_control");

        while let Some(delivery_res) = consumer.next().await {
            let delivery = match delivery_res {
                Ok(d) => d,
                Err(e) => {
                    error!("Error receiving delivery: {:?}", e);
                    continue;
                }
            };

            let payload = match serde_json::from_slice::<AudioEngineRequest>(&delivery.data) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to deserialize request: {:?}", e);
                    let _ = delivery.ack(BasicAckOptions::default()).await;
                    continue;
                }
            };

            // Get retry count
            let mut retry_count = 0;
            if let Some(headers) = delivery.properties.headers() {
                if let Some(v) = headers.inner().get("x-retry-count") {
                    if let AMQPValue::LongLongInt(c) = v {
                        retry_count = *c as u32;
                    }
                }
            }

            let reply_to = delivery
                .properties
                .reply_to()
                .as_ref()
                .map(|s| s.to_string());
            let correlation_id = delivery
                .properties
                .correlation_id()
                .as_ref()
                .map(|s| s.to_string());

            let mut skipped = false;

            match &payload {
                AudioEngineRequest::Join {
                    guild_id,
                    channel_id,
                } => {
                    if self.session_manager.get_session(*guild_id).is_some() {
                        skipped = true;
                    } else {
                        match self.session_manager.join(*guild_id, *channel_id).await {
                            Ok(_) => {
                                self.spawn_session_consumer(conn.clone(), *guild_id, *channel_id)
                                    .await;
                                self.send_reply(
                                    &channel,
                                    reply_to.clone(),
                                    correlation_id.clone(),
                                    AudioEngineResponse::SuccessBool(true),
                                )
                                .await;
                            }
                            Err(_e) => {
                                skipped = true;
                            }
                        }
                    }
                }
                AudioEngineRequest::Leave {
                    guild_id,
                    channel_id,
                } => {
                    if let Some(session) = self.session_manager.get_session(*guild_id) {
                        // Check if we are in the correct channel
                        let state = session.session_state().await;
                        if let Ok(Some(s)) = state {
                            if s.channel_id == *channel_id {
                                match self.session_manager.leave(*guild_id).await {
                                    Ok(_) => {
                                        if let Some((_, handle)) =
                                            self.session_consumers.remove(guild_id)
                                        {
                                            handle.abort();
                                        }
                                        self.send_reply(
                                            &channel,
                                            reply_to.clone(),
                                            correlation_id.clone(),
                                            AudioEngineResponse::SuccessBool(true),
                                        )
                                        .await;
                                    }
                                    Err(e) => {
                                        self.send_reply(
                                            &channel,
                                            reply_to.clone(),
                                            correlation_id.clone(),
                                            AudioEngineResponse::Error(e.to_string()),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                skipped = true;
                            }
                        } else {
                            skipped = true;
                        }
                    } else {
                        skipped = true;
                    }
                }
                _ => {
                    // Session specific requests should not be on the control queue
                    error!(
                        "Received session-specific request on control queue: {:?}",
                        payload
                    );
                    self.send_reply(
                        &channel,
                        reply_to.clone(),
                        correlation_id.clone(),
                        AudioEngineResponse::Error(
                            "Request must be sent to session queue".to_string(),
                        ),
                    )
                    .await;
                }
            }

            if skipped {
                tracing::debug!(
                    "No eligible Audio Engine found for request {:?} (retry count: {})",
                    payload,
                    retry_count
                );
                if retry_count >= self.max_retries {
                    self.send_reply(
                        &channel,
                        reply_to.clone(),
                        correlation_id.clone(),
                        AudioEngineResponse::Error(
                            "Max retries exceeded, no eligible Audio Engine found".to_string(),
                        ),
                    )
                    .await;
                    let _ = delivery.ack(BasicAckOptions::default()).await;
                } else {
                    // Clone and requeue
                    let mut new_headers = FieldTable::default();
                    if let Some(headers) = delivery.properties.headers() {
                        new_headers = headers.clone();
                    }
                    new_headers.insert(
                        ShortString::from("x-retry-count"),
                        AMQPValue::LongLongInt((retry_count + 1) as i64),
                    );

                    let mut props = delivery.properties.clone();
                    props = props.with_headers(new_headers);

                    let _ = channel
                        .basic_publish(
                            "".into(),
                            "audio_engine_control".into(),
                            BasicPublishOptions::default(),
                            &delivery.data,
                            props,
                        )
                        .await;

                    let _ = delivery.ack(BasicAckOptions::default()).await;
                }
            } else {
                let _ = delivery.ack(BasicAckOptions::default()).await;
            }
        }

        Ok(())
    }

    async fn send_reply(
        &self,
        channel: &Channel,
        reply_to: Option<String>,
        correlation_id: Option<String>,
        response: AudioEngineResponse,
    ) {
        if let (Some(rt), Some(cid)) = (reply_to, correlation_id) {
            let data = serde_json::to_vec(&response).unwrap_or_default();
            let _ = channel
                .basic_publish(
                    "".into(),
                    rt.as_str().into(),
                    BasicPublishOptions::default(),
                    &data,
                    BasicProperties::default().with_correlation_id(cid.into()),
                )
                .await;
        }
    }

    async fn spawn_session_consumer(
        &self,
        conn: Arc<Connection>,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) {
        let session_manager = self.session_manager.clone();
        let queue_name = format!("audio_engine_session_{}_{}", guild_id, channel_id);

        let handle = tokio::spawn(async move {
            let channel = match conn.create_channel().await {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to create channel for session {}: {:?}", guild_id, e);
                    return;
                }
            };

            let _ = match channel
                .queue_declare(
                    queue_name.as_str().into(),
                    QueueDeclareOptions {
                        exclusive: true,
                        auto_delete: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
            {
                Ok(q) => q,
                Err(e) => {
                    error!("Failed to declare session queue {}: {:?}", queue_name, e);
                    return;
                }
            };

            let mut consumer = match channel
                .basic_consume(
                    queue_name.as_str().into(),
                    format!("consumer_{}", queue_name).as_str().into(),
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to consume session queue {}: {:?}", queue_name, e);
                    return;
                }
            };

            info!("Started session consumer for {}", queue_name);

            while let Some(delivery_res) = consumer.next().await {
                let delivery = match delivery_res {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let payload = match serde_json::from_slice::<AudioEngineRequest>(&delivery.data) {
                    Ok(p) => p,
                    Err(_) => {
                        let _ = delivery.ack(BasicAckOptions::default()).await;
                        continue;
                    }
                };

                let reply_to = delivery
                    .properties
                    .reply_to()
                    .as_ref()
                    .map(|s| s.to_string());
                let correlation_id = delivery
                    .properties
                    .correlation_id()
                    .as_ref()
                    .map(|s| s.to_string());

                // Self-healing check
                if session_manager.get_session(guild_id).is_none() {
                    warn!(
                        "Session {} no longer managed by this AE. Aborting consumer.",
                        guild_id
                    );
                    if let (Some(rt), Some(cid)) = (&reply_to, &correlation_id) {
                        let resp = AudioEngineResponse::Error(
                            "Session not found (Self-Healed)".to_string(),
                        );
                        let data = serde_json::to_vec(&resp).unwrap();
                        let _ = channel
                            .basic_publish(
                                "".into(),
                                rt.as_str().into(),
                                BasicPublishOptions::default(),
                                &data,
                                BasicProperties::default().with_correlation_id(cid.clone().into()),
                            )
                            .await;
                    }
                    let _ = delivery.ack(BasicAckOptions::default()).await;
                    break;
                }

                let response = match payload {
                    AudioEngineRequest::Play {
                        queue_name,
                        tap_name,
                        audio_request_string,
                        volume,
                        discord_user_id,
                        ..
                    } => {
                        let session = session_manager.get_session(guild_id).unwrap();
                        match session
                            .play(
                                queue_name,
                                tap_name,
                                audio_request_string,
                                volume,
                                discord_user_id,
                            )
                            .await
                        {
                            Ok(tid) => AudioEngineResponse::SuccessTrackId(tid),
                            Err(e) => AudioEngineResponse::Error(e.to_string()),
                        }
                    }
                    AudioEngineRequest::SetVolume {
                        track_id, volume, ..
                    } => {
                        let session = session_manager.get_session(guild_id).unwrap();
                        match session.set_volume(track_id, volume).await {
                            Ok(_) => AudioEngineResponse::SuccessBool(true),
                            Err(e) => AudioEngineResponse::Error(e.to_string()),
                        }
                    }
                    AudioEngineRequest::Stop { track_id, .. } => {
                        let session = session_manager.get_session(guild_id).unwrap();
                        match session.stop(track_id).await {
                            Ok(_) => AudioEngineResponse::SuccessBool(true),
                            Err(e) => AudioEngineResponse::Error(e.to_string()),
                        }
                    }
                    AudioEngineRequest::StopMany { filter, .. } => {
                        let session = session_manager.get_session(guild_id).unwrap();
                        match session.stop_many(filter).await {
                            Ok(_) => AudioEngineResponse::SuccessBool(true),
                            Err(e) => AudioEngineResponse::Error(e.to_string()),
                        }
                    }
                    AudioEngineRequest::NextMusic { .. } => {
                        let session = session_manager.get_session(guild_id).unwrap();
                        match session.next_music().await {
                            Ok(_) => AudioEngineResponse::SuccessBool(true),
                            Err(e) => AudioEngineResponse::Error(e.to_string()),
                        }
                    }
                    AudioEngineRequest::GetSessionState { .. } => {
                        let session = session_manager.get_session(guild_id).unwrap();
                        match session.session_state().await {
                            Ok(Some(state)) => AudioEngineResponse::SuccessSessionState(state),
                            Ok(None) => AudioEngineResponse::Error("State not found".to_string()),
                            Err(e) => AudioEngineResponse::Error(e.to_string()),
                        }
                    }
                    _ => {
                        AudioEngineResponse::Error("Invalid request for session queue".to_string())
                    }
                };

                if let (Some(rt), Some(cid)) = (reply_to, correlation_id) {
                    let data = serde_json::to_vec(&response).unwrap();
                    let _ = channel
                        .basic_publish(
                            "".into(),
                            rt.as_str().into(),
                            BasicPublishOptions::default(),
                            &data,
                            BasicProperties::default().with_correlation_id(cid.into()),
                        )
                        .await;
                }

                let _ = delivery.ack(BasicAckOptions::default()).await;
            }
            info!("Session consumer for {} stopped", queue_name);
        });

        self.session_consumers.insert(guild_id, handle);
    }
}
