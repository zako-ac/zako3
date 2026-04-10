use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand,
};
use tracing::{error, warn};
use zako3_ae_transport::TlClientHandler;

use zako3_audio_engine_core::engine::session_manager::SessionManager;

pub struct AeTransportHandler {
    pub session_manager: Arc<SessionManager>,
}

impl AeTransportHandler {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[async_trait]
impl TlClientHandler for AeTransportHandler {
    async fn handle(
        &self,
        req: AudioEngineCommandRequest,
        _headers: &HashMap<String, String>,
    ) -> AudioEngineCommandResponse {
        let guild_id = req.session.guild_id;
        let channel_id = req.session.channel_id;

        match req.command {
            AudioEngineCommand::Join => {
                if self
                    .session_manager
                    .get_session(guild_id, channel_id)
                    .is_some()
                {
                    return AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined);
                }
                match self.session_manager.join(guild_id, channel_id).await {
                    Ok(_) => AudioEngineCommandResponse::Ok,
                    Err(e) => err(&e.to_string()),
                }
            }

            AudioEngineCommand::SessionCommand(cmd) => {
                let Some(session) = self.session_manager.get_session(guild_id, channel_id) else {
                    warn!(guild_id = ?guild_id, channel_id = ?channel_id, "Session not found");
                    return AudioEngineCommandResponse::Error(AudioEngineError::NotJoined);
                };

                match cmd {
                    AudioEngineSessionCommand::Leave => {
                        match self.session_manager.leave(guild_id, channel_id).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::Play(play_req) => {
                        match session
                            .play(
                                play_req.queue_name,
                                play_req.tap_name,
                                play_req.ars,
                                play_req.volume,
                                play_req.initiator,
                            )
                            .await
                        {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::Stop(track_id) => {
                        match session.stop(track_id).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::StopMany(filter) => {
                        match session.stop_many(filter).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::SetVolume { track_id, volume } => {
                        match session.set_volume(track_id, volume).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::NextMusic => match session.next_music().await {
                        Ok(_) => AudioEngineCommandResponse::Ok,
                        Err(e) => err(&e.to_string()),
                    },

                    AudioEngineSessionCommand::Pause(queue_name) => {
                        match session.pause_queue(queue_name).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::Resume(queue_name) => {
                        match session.resume_queue(queue_name).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::GetSessionState => {
                        match session.session_state().await {
                            Ok(Some(state)) => AudioEngineCommandResponse::SessionState(state),
                            Ok(None) => err("State not found"),
                            Err(e) => err(&e.to_string()),
                        }
                    }
                }
            }
        }
    }
}

fn err(msg: &str) -> AudioEngineCommandResponse {
    error!(msg, "AudioEngine command error");
    AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg.to_string()))
}
