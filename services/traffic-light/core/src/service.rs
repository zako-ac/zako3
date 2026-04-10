use std::sync::Arc;

use tl_protocol::{
    AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, SessionInfo, AudioEngineCommand,
};
use zako3_types::{GuildId, SessionState};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::{AeDispatcher, RouterError, RouterResult, StateChangeEvent, ZakoState, router};

pub struct TlService {
    state: Arc<RwLock<ZakoState>>,
    dispatcher: Arc<dyn AeDispatcher>,
}

impl TlService {
    pub fn new(state: Arc<RwLock<ZakoState>>, dispatcher: Arc<dyn AeDispatcher>) -> Self {
        Self { state, dispatcher }
    }

    pub fn state(&self) -> Arc<RwLock<ZakoState>> {
        self.state.clone()
    }

    #[tracing::instrument(
        skip(self),
        fields(
            guild_id = %request.session.guild_id,
            channel_id = %request.session.channel_id,
            idempotency_key = ?request.idempotency_key,
        )
    )]
    pub async fn execute(&self, request: AudioEngineCommandRequest) -> AudioEngineCommandResponse {
        let route_result = {
            let state = self.state.read().await;
            router::route(&state, &request)
        };

        match route_result {
            Err(RouterError::NotJoined) => {
                if matches!(
                    request.command,
                    AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave)
                ) {
                    // Broadcast Leave to all routes known for this guild — best-effort
                    // cleanup in case TL state drifted and lost track of the session.
                    let routes = {
                        let state = self.state.read().await;
                        state
                            .sessions_by_guild_id(request.session.guild_id)
                            .into_iter()
                            .map(|(route, _)| route)
                            .collect::<Vec<_>>()
                    };
                    for route in routes {
                        if let Err(e) = self.dispatcher.send(route, request.clone()).await {
                            warn!(error = %e, "broadcast Leave dispatch failed");
                        }
                    }
                    AudioEngineCommandResponse::Ok
                } else {
                    warn!("Routing failed: session not joined");
                    AudioEngineCommandResponse::Error(AudioEngineError::NotJoined)
                }
            }
            Err(RouterError::NoAvailableWorker) => {
                warn!("Routing failed: no available worker for guild");
                AudioEngineCommandResponse::Error(AudioEngineError::InternalError(
                    "No available worker for this guild".into(),
                ))
            }
            Ok(RouterResult::Join(candidates)) => {
                for candidate in candidates {
                    info!(
                        worker_id = candidate.route.worker_id.0,
                        ae_id = candidate.route.ae_id.0,
                        "Trying Join on AE"
                    );
                    match self.dispatcher.send(candidate.route, request.clone()).await {
                        Ok(resp) if matches!(
                            &resp,
                            AudioEngineCommandResponse::Ok
                                | AudioEngineCommandResponse::Error(
                                    AudioEngineError::AlreadyJoined
                                )
                        ) => {
                            // AlreadyJoined from AE means the session is active there —
                            // treat as success (covers state-drift resync joins).
                            let mut state = self.state.write().await;
                            *state = candidate.new_state_on_success;
                            return AudioEngineCommandResponse::Ok;
                        }
                        Ok(err_resp) => {
                            warn!(
                                worker_id = candidate.route.worker_id.0,
                                ae_id = candidate.route.ae_id.0,
                                "Join failed on AE: {err_resp:?}, trying next candidate"
                            );
                        }
                        Err(e) => {
                            warn!(
                                worker_id = candidate.route.worker_id.0,
                                ae_id = candidate.route.ae_id.0,
                                error = %e,
                                "Dispatch error on Join, trying next candidate"
                            );
                        }
                    }
                }
                error!("All workers failed to handle Join");
                AudioEngineCommandResponse::Error(AudioEngineError::InternalError(
                    "All workers failed to handle Join".into(),
                ))
            }
            Ok(RouterResult::Session(success)) => {
                let route = success.route;
                info!(
                    worker_id = route.worker_id.0,
                    ae_id = route.ae_id.0,
                    "Routing to AE"
                );

                match self.dispatcher.send(route, request).await {
                    Ok(response) => {
                        if !matches!(response, AudioEngineCommandResponse::Error(_)) {
                            let mut state = self.state.write().await;
                            *state = success.new_state_on_success;
                        }
                        response
                    }
                    Err(e) => {
                        error!(error = %e, "AE dispatch failed");
                        AudioEngineCommandResponse::Error(AudioEngineError::InternalError(
                            e.to_string(),
                        ))
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self), fields(event_type = "VoiceStateUpdate"))]
    pub async fn handle_state_change(&self, event: StateChangeEvent) {
        match event {
            StateChangeEvent::VoiceStateUpdate(e) => {
                let span = tracing::Span::current();
                span.record("guild_id", tracing::field::debug(e.guild_id));
                span.record("user_id", tracing::field::debug(&e.user_id));

                if e.after.is_some() {
                    return;
                }

                let (worker_id, sessions_to_leave) = {
                    let state = self.state.read().await;
                    let Some(worker_id) = state.worker_by_bot_client_id(&e.user_id) else {
                        return;
                    };
                    let sessions = state
                        .sessions_by_worker(worker_id)
                        .into_iter()
                        .filter(|(_, info)| info.guild_id == e.guild_id)
                        .collect::<Vec<_>>();
                    (worker_id, sessions)
                };

                for (route, session_info) in sessions_to_leave {
                    info!(
                        worker_id = worker_id.0,
                        guild_id = ?e.guild_id,
                        channel_id = ?session_info.channel_id,
                        "Auto-triggering Leave due to voice state change"
                    );
                    let leave_req = AudioEngineCommandRequest {
                        session: SessionInfo {
                            guild_id: session_info.guild_id,
                            channel_id: session_info.channel_id,
                        },
                        command: AudioEngineCommand::SessionCommand(
                            AudioEngineSessionCommand::Leave,
                        ),
                        headers: std::collections::HashMap::new(),
                        idempotency_key: None,
                    };
                    if let Err(e) = self.dispatcher.send(route, leave_req).await {
                        error!(error = %e, "Failed to dispatch auto-Leave");
                    } else {
                        let mut state = self.state.write().await;
                        state.sessions.remove(&route);
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self), fields(guild_id = ?guild_id))]
    pub async fn get_sessions_in_guild(&self, guild_id: GuildId) -> Vec<SessionState> {
        let sessions = {
            let state = self.state.read().await;
            state.sessions_by_guild_id(guild_id)
        };

        let mut results = Vec::new();
        for (route, session_info) in sessions {
            let req = AudioEngineCommandRequest {
                session: SessionInfo {
                    guild_id: session_info.guild_id,
                    channel_id: session_info.channel_id,
                },
                command: tl_protocol::AudioEngineCommand::SessionCommand(
                    tl_protocol::AudioEngineSessionCommand::GetSessionState,
                ),
                headers: std::collections::HashMap::new(),
                idempotency_key: None,
            };
            match self.dispatcher.send(route, req).await {
                Ok(AudioEngineCommandResponse::SessionState(s)) => results.push(s),
                Ok(other) => warn!(?route, "unexpected response for GetSessionState: {other:?}"),
                Err(e) => warn!(?route, error = %e, "failed to get session state"),
            }
        }
        results
    }

    pub async fn report_guilds(&self, token: String, guilds: Vec<GuildId>) {
        let mut state = self.state.write().await;
        if let Some(worker) = state.workers.values_mut().find(|w| w.discord_token.0 == token) {
            info!(token, guild_count = guilds.len(), "Updating worker guild permissions");
            worker.permissions.set_allowed_guilds(guilds);
        } else {
            warn!(token, "report_guilds: no worker found for token");
        }
    }
}
