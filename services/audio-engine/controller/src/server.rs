use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use opentelemetry::global;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineRpcServer, AudioEngineSessionCommand,
};
use tracing::{Instrument as _, error, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use zako3_audio_engine_core::engine::session_manager::SessionManager;

pub struct AeTransportHandler {
    pub session_manager: Arc<OnceLock<Arc<SessionManager>>>,
}

impl AeTransportHandler {
    pub fn new(session_manager: Arc<OnceLock<Arc<SessionManager>>>) -> Self {
        Self { session_manager }
    }
}

#[async_trait]
impl AudioEngineRpcServer for AeTransportHandler {
    async fn execute(
        &self,
        req: AudioEngineCommandRequest,
    ) -> RpcResult<AudioEngineCommandResponse> {
        let parent_cx = global::get_text_map_propagator(|p| p.extract(&req.headers));
        let cmd = command_name(&req.command);
        let span = tracing::info_span!("ae.execute", command = cmd);
        let _ = span.set_parent(parent_cx);

        let response = self.handle_command(req).instrument(span).await;
        Ok(response)
    }
}

impl AeTransportHandler {
    async fn handle_command(&self, req: AudioEngineCommandRequest) -> AudioEngineCommandResponse {
        let session_manager = match self.session_manager.get() {
            Some(sm) => sm.clone(),
            None => return err("Audio Engine not yet initialized"),
        };

        match req.command {
            AudioEngineCommand::FetchDiscordVoiceState => {
                use tl_protocol::SessionInfo;
                match session_manager.fetch_discord_voice_state().await {
                    Ok(voice_states) => {
                        let sessions: Vec<SessionInfo> = voice_states
                            .into_iter()
                            .map(|(guild_id, channel_id)| SessionInfo {
                                guild_id,
                                channel_id,
                            })
                            .collect();
                        AudioEngineCommandResponse::DiscordVoiceState(sessions)
                    }
                    Err(e) => err(&e.to_string()),
                }
            }
            AudioEngineCommand::Join => {
                let Some(session_info) = req.session else {
                    return err("session required for Join");
                };
                if session_manager
                    .get_session(session_info.guild_id, session_info.channel_id)
                    .is_some()
                {
                    return AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined);
                }
                match session_manager
                    .join(session_info.guild_id, session_info.channel_id)
                    .await
                {
                    Ok(_) => AudioEngineCommandResponse::Ok,
                    Err(e) => err(&e.to_string()),
                }
            }

            AudioEngineCommand::SessionCommand(cmd) => {
                let Some(session_info) = req.session else {
                    return err("session required for SessionCommand");
                };
                let guild_id = session_info.guild_id;
                let channel_id = session_info.channel_id;
                let Some(session) = session_manager.get_session(guild_id, channel_id) else {
                    warn!(guild_id = ?guild_id, channel_id = ?channel_id, "Session not found");
                    return AudioEngineCommandResponse::Error(AudioEngineError::NotJoined);
                };

                match cmd {
                    AudioEngineSessionCommand::Leave => {
                        match session_manager.leave(guild_id, channel_id).await {
                            Ok(_) => AudioEngineCommandResponse::Ok,
                            Err(e) => err(&e.to_string()),
                        }
                    }

                    AudioEngineSessionCommand::Play(play_req) => {
                        match session
                            .play(
                                play_req.queue_name,
                                play_req.tap_id,
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

fn command_name(cmd: &AudioEngineCommand) -> &'static str {
    match cmd {
        AudioEngineCommand::Join => "join",
        AudioEngineCommand::FetchDiscordVoiceState => "fetch_discord_voice_state",
        AudioEngineCommand::SessionCommand(sc) => match sc {
            AudioEngineSessionCommand::Leave => "leave",
            AudioEngineSessionCommand::Play(_) => "play",
            AudioEngineSessionCommand::Stop(_) => "stop",
            AudioEngineSessionCommand::StopMany(_) => "stop_many",
            AudioEngineSessionCommand::SetVolume { .. } => "set_volume",
            AudioEngineSessionCommand::NextMusic => "next_music",
            AudioEngineSessionCommand::Pause(_) => "pause",
            AudioEngineSessionCommand::Resume(_) => "resume",
            AudioEngineSessionCommand::GetSessionState => "get_session_state",
        },
    }
}

fn err(msg: &str) -> AudioEngineCommandResponse {
    error!(msg, "AudioEngine command error");
    AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, OnceLock};
    use tl_protocol::SessionInfo;
    use zako3_audio_engine_core::engine::session_manager::SessionManager;
    use zako3_audio_engine_core::service::discord::MockDiscordService;
    use zako3_audio_engine_core::service::state::MockStateService;
    use zako3_audio_engine_core::service::taphub::MockTapHubService;
    use zako3_audio_engine_core::types::{ChannelId, GuildId};

    #[tokio::test]
    async fn handler_fetch_discord_voice_state_returns_sessions() {
        let guild_id = GuildId::from(1);
        let channel_id = ChannelId::from(100);

        let mut mock_discord = MockDiscordService::new();
        let mock_state = MockStateService::new();
        let mock_taphub = MockTapHubService::new();

        mock_discord
            .expect_get_active_voice_connections()
            .times(1)
            .returning(move || Ok(vec![(guild_id, channel_id)]));

        let session_manager = Arc::new(SessionManager::new(
            Arc::new(mock_discord),
            Arc::new(mock_state),
            Arc::new(mock_taphub),
        ));

        let sm_cell = Arc::new(OnceLock::new());
        let _ = sm_cell.set(session_manager);

        let handler = AeTransportHandler::new(sm_cell);
        let req = AudioEngineCommandRequest {
            session: None,
            command: AudioEngineCommand::FetchDiscordVoiceState,
            headers: HashMap::new(),
            idempotency_key: None,
        };

        let resp = handler.handle_command(req).await;

        match resp {
            AudioEngineCommandResponse::DiscordVoiceState(sessions) => {
                assert_eq!(sessions.len(), 1);
                assert_eq!(
                    sessions[0],
                    SessionInfo {
                        guild_id: GuildId::from(1),
                        channel_id: ChannelId::from(100),
                    }
                );
            }
            _ => panic!("Expected DiscordVoiceState response"),
        }
    }

    #[tokio::test]
    async fn handler_fetch_discord_voice_state_empty() {
        let mut mock_discord = MockDiscordService::new();
        let mock_state = MockStateService::new();
        let mock_taphub = MockTapHubService::new();

        mock_discord
            .expect_get_active_voice_connections()
            .times(1)
            .returning(|| Ok(vec![]));

        let session_manager = Arc::new(SessionManager::new(
            Arc::new(mock_discord),
            Arc::new(mock_state),
            Arc::new(mock_taphub),
        ));

        let sm_cell = Arc::new(OnceLock::new());
        let _ = sm_cell.set(session_manager);

        let handler = AeTransportHandler::new(sm_cell);
        let req = AudioEngineCommandRequest {
            session: None,
            command: AudioEngineCommand::FetchDiscordVoiceState,
            headers: HashMap::new(),
            idempotency_key: None,
        };

        let resp = handler.handle_command(req).await;

        match resp {
            AudioEngineCommandResponse::DiscordVoiceState(sessions) => {
                assert_eq!(sessions.len(), 0);
            }
            _ => panic!("Expected DiscordVoiceState response"),
        }
    }

    #[tokio::test]
    async fn handler_fetch_discord_voice_state_error() {
        use std::io;

        let mut mock_discord = MockDiscordService::new();
        let mock_state = MockStateService::new();
        let mock_taphub = MockTapHubService::new();

        mock_discord
            .expect_get_active_voice_connections()
            .times(1)
            .returning(|| Err(io::Error::new(io::ErrorKind::Other, "Discord error").into()));

        let session_manager = Arc::new(SessionManager::new(
            Arc::new(mock_discord),
            Arc::new(mock_state),
            Arc::new(mock_taphub),
        ));

        let sm_cell = Arc::new(OnceLock::new());
        let _ = sm_cell.set(session_manager);

        let handler = AeTransportHandler::new(sm_cell);
        let req = AudioEngineCommandRequest {
            session: None,
            command: AudioEngineCommand::FetchDiscordVoiceState,
            headers: HashMap::new(),
            idempotency_key: None,
        };

        let resp = handler.handle_command(req).await;

        match resp {
            AudioEngineCommandResponse::Error(_) => {
                // Expected
            }
            _ => panic!("Expected Error response"),
        }
    }
}
