use std::sync::Arc;

use async_nats::Client;
use dashmap::DashMap;
use futures_util::StreamExt;
use tracing::{error, info, warn};

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_core::types::{ChannelId, GuildId};

use zako3_audio_engine_client::{AudioEngineRequest, AudioEngineResponse};

pub struct AudioEngineServer {
    pub session_manager: Arc<SessionManager>,
    pub nats_url: String,
    pub session_consumers: DashMap<GuildId, tokio::task::JoinHandle<()>>,
}

impl AudioEngineServer {
    pub fn new(session_manager: Arc<SessionManager>, nats_url: String) -> Self {
        Self {
            session_manager,
            nats_url,
            session_consumers: DashMap::new(),
        }
    }

    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        let client = async_nats::connect(&self.nats_url).await?;

        (*self).reconnect_sessions(&client).await;

        let mut sub = client
            .queue_subscribe("audio_engine.control", "audio_engine_control".to_string())
            .await?;

        info!("Audio Engine listening for control messages on audio_engine.control");

        while let Some(msg) = sub.next().await {
            let payload = match serde_json::from_slice::<AudioEngineRequest>(&msg.payload) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to deserialize request: {:?}", e);
                    continue;
                }
            };

            let reply = match msg.reply {
                Some(r) => r,
                None => {
                    error!("Received control message without reply subject");
                    continue;
                }
            };

            let this = Arc::clone(&self);
            let client = client.clone();
            tokio::spawn(async move {
                let response = match &payload {
                    AudioEngineRequest::Join { guild_id, channel_id } => {
                        if this.session_manager.get_session(*guild_id).is_some() {
                            AudioEngineResponse::Error("Session already exists".to_string())
                        } else {
                            match this.session_manager.join(*guild_id, *channel_id).await {
                                Ok(_) => {
                                    this.spawn_session_consumer(client.clone(), *guild_id, *channel_id)
                                        .await;
                                    AudioEngineResponse::SuccessBool(true)
                                }
                                Err(e) => AudioEngineResponse::Error(e.to_string()),
                            }
                        }
                    }
                    AudioEngineRequest::Leave { guild_id, channel_id } => {
                        if let Some(session) = this.session_manager.get_session(*guild_id) {
                            let state = session.session_state().await;
                            if let Ok(Some(s)) = state {
                                if s.channel_id == *channel_id {
                                    match this.session_manager.leave(*guild_id).await {
                                        Ok(_) => {
                                            if let Some((_, handle)) =
                                                this.session_consumers.remove(guild_id)
                                            {
                                                handle.abort();
                                            }
                                            AudioEngineResponse::SuccessBool(true)
                                        }
                                        Err(e) => AudioEngineResponse::Error(e.to_string()),
                                    }
                                } else {
                                    AudioEngineResponse::Error("Not in that channel".to_string())
                                }
                            } else {
                                AudioEngineResponse::Error("No session state".to_string())
                            }
                        } else {
                            AudioEngineResponse::Error("No session found".to_string())
                        }
                    }
                    _ => AudioEngineResponse::Error(
                        "Request must be sent to session subject".to_string(),
                    ),
                };

                let data = serde_json::to_vec(&response).unwrap_or_default();
                let _ = client.publish(reply, data.into()).await;
            });
        }

        Ok(())
    }

    async fn reconnect_sessions(&self, client: &Client) {
        let sessions = match self.session_manager.list_sessions().await {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to list sessions for reconnect: {:?}", e);
                return;
            }
        };

        if sessions.is_empty() {
            return;
        }

        info!("Rejoining {} session(s) from previous run", sessions.len());
        for session in sessions {
            let guild_id = session.guild_id;
            let channel_id = session.channel_id;

            if let Err(e) = self.session_manager.rejoin(&session).await {
                warn!(guild_id = ?guild_id, "Failed to rejoin session: {:?}", e);
                continue;
            }

            self.spawn_session_consumer(client.clone(), guild_id, channel_id)
                .await;
            info!(guild_id = ?guild_id, "Rejoined session");
        }
    }

    async fn spawn_session_consumer(
        &self,
        client: Client,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) {
        let session_manager = self.session_manager.clone();
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);

        let handle = tokio::spawn(async move {
            let mut sub = match client.subscribe(subject.clone()).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to subscribe to session subject {}: {:?}", subject, e);
                    return;
                }
            };

            info!("Started session consumer for {}", subject);

            while let Some(msg) = sub.next().await {
                let payload = match serde_json::from_slice::<AudioEngineRequest>(&msg.payload) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let reply = match msg.reply {
                    Some(r) => r,
                    None => continue,
                };

                let session_manager = session_manager.clone();
                let client = client.clone();
                tokio::spawn(async move {
                    // Self-healing check
                    if session_manager.get_session(guild_id).is_none() {
                        warn!(
                            "Session {} no longer managed by this AE. Aborting consumer.",
                            guild_id
                        );
                        let resp = AudioEngineResponse::Error("Session not found (Self-Healed)".to_string());
                        let data = serde_json::to_vec(&resp).unwrap_or_default();
                        let _ = client.publish(reply, data.into()).await;
                        return;
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
                                .play(queue_name, tap_name, audio_request_string, volume, discord_user_id)
                                .await
                            {
                                Ok(tid) => AudioEngineResponse::SuccessTrackId(tid),
                                Err(e) => AudioEngineResponse::Error(e.to_string()),
                            }
                        }
                        AudioEngineRequest::SetVolume { track_id, volume, .. } => {
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
                        _ => AudioEngineResponse::Error("Invalid request for session subject".to_string()),
                    };

                    let data = serde_json::to_vec(&response).unwrap_or_default();
                    let _ = client.publish(reply, data.into()).await;

                    // Publish state-changed event for mutating operations that succeeded
                    let is_mutation = matches!(
                        response,
                        AudioEngineResponse::SuccessBool(true) | AudioEngineResponse::SuccessTrackId(_)
                    );
                    if is_mutation {
                        let event = serde_json::json!({
                            "guild_id": guild_id,
                            "channel_id": channel_id
                        });
                        let subject = format!("playback.state_changed.{}.{}", guild_id, channel_id);
                        if let Ok(payload) = serde_json::to_vec(&event) {
                            let _ = client.publish(subject, payload.into()).await;
                        }
                    }
                });
            }

            info!("Session consumer for {} stopped", subject);
        });

        self.session_consumers.insert(guild_id, handle);
    }
}
