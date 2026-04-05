use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};

use lapin::{
    BasicProperties, Connection, ConnectionProperties, Channel,
    options::{BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
};
use futures_util::StreamExt;

use zako3_audio_engine_core::types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TapName,
    TrackId, Volume, hq::DiscordUserId,
};

use crate::{AudioEngineRequest, AudioEngineResponse};

pub struct AudioEngineRpcClient {
    channel: Channel,
    reply_queue: String,
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<AudioEngineResponse>>>>,
}

impl AudioEngineRpcClient {
    pub async fn new(rabbitmq_url: &str) -> anyhow::Result<Self> {
        let conn = Connection::connect(rabbitmq_url, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?;

        let reply_queue = channel
            .queue_declare(
                "".into(),
                QueueDeclareOptions {
                    exclusive: true,
                    auto_delete: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        let reply_queue_name = reply_queue.name().to_string();

        let pending_requests = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<AudioEngineResponse>>::new()));
        
        let mut consumer = channel
            .basic_consume(
                reply_queue_name.as_str().into(),
                "client_reply_consumer".into(),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let pr = pending_requests.clone();
        tokio::spawn(async move {
            while let Some(delivery_res) = consumer.next().await {
                if let Ok(delivery) = delivery_res {
                    if let Some(corr_id) = delivery.properties.correlation_id() {
                        let cid = corr_id.to_string();
                        if let Ok(response) = serde_json::from_slice::<AudioEngineResponse>(&delivery.data) {
                            let mut map = pr.lock().await;
                            if let Some(sender) = map.remove(&cid) {
                                let _ = sender.send(response);
                            }
                        }
                    }
                    let _ = delivery.ack(lapin::options::BasicAckOptions::default()).await;
                }
            }
        });

        Ok(Self {
            channel,
            reply_queue: reply_queue_name,
            pending_requests,
        })
    }

    async fn send_request(
        &self,
        routing_key: &str,
        request: AudioEngineRequest,
    ) -> anyhow::Result<AudioEngineResponse> {
        let corr_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::to_vec(&request)?;

        let (tx, rx) = oneshot::channel();
        self.pending_requests.lock().await.insert(corr_id.clone(), tx);

        self.channel
            .basic_publish(
                "".into(),
                routing_key.into(),
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_reply_to(self.reply_queue.clone().into())
                    .with_correlation_id(corr_id.clone().into()),
            )
            .await?;

        let response = rx.await?;
        Ok(response)
    }

    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> anyhow::Result<bool> {
        match self.send_request("audio_engine_control", AudioEngineRequest::Join { guild_id, channel_id }).await? {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> anyhow::Result<bool> {
        match self.send_request("audio_engine_control", AudioEngineRequest::Leave { guild_id, channel_id }).await? {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn play(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_name: TapName,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    ) -> anyhow::Result<TrackId> {
        let routing_key = format!("audio_engine_session_{}_{}", guild_id, channel_id);
        match self.send_request(&routing_key, AudioEngineRequest::Play { guild_id, queue_name, tap_name, audio_request_string, volume, discord_user_id }).await? {
            AudioEngineResponse::SuccessTrackId(id) => Ok(id),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn set_volume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
        volume: Volume,
    ) -> anyhow::Result<bool> {
        let routing_key = format!("audio_engine_session_{}_{}", guild_id, channel_id);
        match self.send_request(&routing_key, AudioEngineRequest::SetVolume { guild_id, track_id, volume }).await? {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn stop(&self, guild_id: GuildId, channel_id: ChannelId, track_id: TrackId) -> anyhow::Result<bool> {
        let routing_key = format!("audio_engine_session_{}_{}", guild_id, channel_id);
        match self.send_request(&routing_key, AudioEngineRequest::Stop { guild_id, track_id }).await? {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn stop_many(&self, guild_id: GuildId, channel_id: ChannelId, filter: AudioStopFilter) -> anyhow::Result<bool> {
        let routing_key = format!("audio_engine_session_{}_{}", guild_id, channel_id);
        match self.send_request(&routing_key, AudioEngineRequest::StopMany { guild_id, filter }).await? {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn next_music(&self, guild_id: GuildId, channel_id: ChannelId) -> anyhow::Result<bool> {
        let routing_key = format!("audio_engine_session_{}_{}", guild_id, channel_id);
        match self.send_request(&routing_key, AudioEngineRequest::NextMusic { guild_id }).await? {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn get_session_state(&self, guild_id: GuildId, channel_id: ChannelId) -> anyhow::Result<SessionState> {
        let routing_key = format!("audio_engine_session_{}_{}", guild_id, channel_id);
        match self.send_request(&routing_key, AudioEngineRequest::GetSessionState { guild_id }).await? {
            AudioEngineResponse::SuccessSessionState(s) => Ok(s),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }
}
