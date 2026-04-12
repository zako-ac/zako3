use std::sync::Arc;

use tl_protocol::{
    AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, SessionInfo, AudioEngineCommand,
};
use zako3_types::{GuildId, SessionState};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::{AeDispatcher, RouterError, RouterResult, SessionRoute, StateChangeEvent, ZakoState, router};

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

    pub async fn execute(&self, request: AudioEngineCommandRequest) -> AudioEngineCommandResponse {
        if let Some(session) = &request.session {
            tracing::debug!(?session, ?request.idempotency_key, "Executing command");
        }
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
                    if let Some(session_info) = &request.session {
                        let routes = {
                            let state = self.state.read().await;
                            state
                                .sessions_by_guild_id(session_info.guild_id)
                                .into_iter()
                                .map(|(route, _)| route)
                                .collect::<Vec<_>>()
                        };
                        for route in routes {
                            if let Err(e) = self.dispatcher.send(route, request.clone()).await {
                                warn!(error = %e, "broadcast Leave dispatch failed");
                            }
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
                        // Sync session state on dispatch failure to clean up stale entries
                        self.sync_sessions().await;
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
                        session: Some(SessionInfo {
                            guild_id: session_info.guild_id,
                            channel_id: session_info.channel_id,
                        }),
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
                session: Some(SessionInfo {
                    guild_id: session_info.guild_id,
                    channel_id: session_info.channel_id,
                }),
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

    /// Reconciles dangling sessions: voice channels where bot is connected in Discord
    /// but has no cached session in AE. Sends Leave for dangling sessions.
    /// Called periodically (every 1 min) and on boot.
    pub async fn reconcile(&self) {
        // Get all connected AE routes from ZakoState
        let all_routes: Vec<SessionRoute> = {
            let state = self.state.read().await;
            state
                .workers
                .iter()
                .flat_map(|(worker_id, worker)| {
                    worker
                        .connected_ae_ids
                        .iter()
                        .map(|&ae_id| SessionRoute {
                            worker_id: *worker_id,
                            ae_id: crate::AeId(ae_id),
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };

        // For each AE route, fetch Discord voice state and compare with cached sessions
        for route in all_routes {
            let req = AudioEngineCommandRequest {
                session: None,
                command: AudioEngineCommand::FetchDiscordVoiceState,
                headers: std::collections::HashMap::new(),
                idempotency_key: None,
            };

            match self.dispatcher.send(route, req).await {
                Ok(AudioEngineCommandResponse::DiscordVoiceState(discord_sessions)) => {
                    // Get cached sessions for this AE
                    let cached: std::collections::HashSet<SessionInfo> = {
                        let state = self.state.read().await;
                        state
                            .sessions
                            .iter()
                            .filter(|(r, _)| *r == &route)
                            .map(|(_, info)| info)
                            .copied()
                            .collect()
                    };

                    // Find dangling sessions: in Discord but not in cache
                    for session_info in discord_sessions {
                        if !cached.contains(&session_info) {
                            tracing::info!(
                                ?route,
                                ?session_info,
                                "Leaving dangling session (in Discord but not cached)"
                            );
                            let leave_req = AudioEngineCommandRequest {
                                session: Some(session_info),
                                command: AudioEngineCommand::SessionCommand(
                                    AudioEngineSessionCommand::Leave,
                                ),
                                headers: std::collections::HashMap::new(),
                                idempotency_key: None,
                            };
                            if let Err(e) = self.dispatcher.send(route, leave_req).await {
                                tracing::warn!(error = %e, "reconcile: failed to leave dangling session");
                            }
                        }
                    }
                }
                Ok(other) => {
                    tracing::warn!(?route, ?other, "reconcile: unexpected response");
                }
                Err(e) => {
                    tracing::warn!(?route, error = %e, "reconcile: dispatch failed");
                }
            }
        }
    }

    /// Fetches current session state from all connected AEs and removes stale sessions.
    /// Called periodically (every 1 min) and on command failures to reconcile state drift.
    pub async fn sync_sessions(&self) {
        // Snapshot all current routes
        let routes: Vec<(SessionRoute, SessionInfo)> = {
            let state = self.state.read().await;
            state
                .sessions
                .iter()
                .map(|(r, i)| (*r, *i))
                .collect()
        };

        if routes.is_empty() {
            return;
        }

        let mut to_remove = Vec::new();

        for (route, session_info) in routes {
            let req = AudioEngineCommandRequest {
                session: Some(session_info),
                command: AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
                headers: std::collections::HashMap::new(),
                idempotency_key: None,
            };

            // Use 5s timeout to avoid blocking on dead AEs
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                self.dispatcher.send(route, req),
            )
            .await;

            match result {
                Ok(Ok(AudioEngineCommandResponse::SessionState(_))) => {
                    // Session is still alive on the AE
                }
                Ok(Ok(other)) => {
                    warn!(
                        ?route,
                        ?other,
                        "sync_sessions: unexpected response, removing stale session"
                    );
                    to_remove.push(route);
                }
                Ok(Err(e)) => {
                    warn!(
                        ?route,
                        error = %e,
                        "sync_sessions: dispatch failed, removing stale session"
                    );
                    to_remove.push(route);
                }
                Err(_timeout) => {
                    warn!(?route, "sync_sessions: timeout, removing stale session");
                    to_remove.push(route);
                }
            }
        }

        if !to_remove.is_empty() {
            let mut state = self.state.write().await;
            for route in to_remove {
                state.sessions.remove(&route);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHashMap;
    use std::sync::Mutex;
    use tl_protocol::{AudioEngineCommand, SessionInfo, AudioEngineSessionCommand};
    use zako3_types::{ChannelId, GuildId};
    use crate::{AeId, DiscordToken, TlError, Worker, WorkerId, WorkerPermissions};

    #[derive(Clone)]
    enum MockCall {
        FetchDiscordVoiceState,
        Leave(SessionInfo),
    }

    struct TestDispatcher {
        calls: Arc<Mutex<Vec<MockCall>>>,
        response: Arc<dyn Fn() -> Result<AudioEngineCommandResponse, TlError> + Send + Sync>,
    }

    #[async_trait::async_trait]
    impl AeDispatcher for TestDispatcher {
        async fn send(
            &self,
            _route: SessionRoute,
            req: AudioEngineCommandRequest,
        ) -> Result<AudioEngineCommandResponse, TlError> {
            match &req.command {
                AudioEngineCommand::FetchDiscordVoiceState => {
                    self.calls.lock().unwrap().push(MockCall::FetchDiscordVoiceState);
                }
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave) => {
                    if let Some(s) = req.session {
                        self.calls.lock().unwrap().push(MockCall::Leave(s));
                    }
                }
                _ => {}
            }
            (self.response)()
        }
    }

    fn route() -> SessionRoute {
        SessionRoute {
            worker_id: WorkerId(0),
            ae_id: AeId(1),
        }
    }

    fn session(g: u64, c: u64) -> SessionInfo {
        SessionInfo {
            guild_id: GuildId::from(g),
            channel_id: ChannelId::from(c),
        }
    }

    fn state_with_no_ae() -> Arc<RwLock<ZakoState>> {
        Arc::new(RwLock::new(ZakoState {
            workers: FxHashMap::default(),
            sessions: Default::default(),
            worker_cursor: 0,
        }))
    }

    fn state_with_connected_ae() -> Arc<RwLock<ZakoState>> {
        let mut workers = FxHashMap::default();
        workers.insert(
            WorkerId(0),
            Worker {
                worker_id: WorkerId(0),
                bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
                discord_token: DiscordToken(String::new()),
                connected_ae_ids: vec![1],
                permissions: WorkerPermissions::new(),
                ae_cursor: 0,
            },
        );
        Arc::new(RwLock::new(ZakoState {
            workers,
            sessions: Default::default(),
            worker_cursor: 0,
        }))
    }

    fn state_with_session(s: SessionInfo) -> Arc<RwLock<ZakoState>> {
        let mut sessions: FxHashMap<SessionRoute, SessionInfo> = Default::default();
        sessions.insert(route(), s);
        let mut workers = FxHashMap::default();
        workers.insert(
            WorkerId(0),
            Worker {
                worker_id: WorkerId(0),
                bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
                discord_token: DiscordToken(String::new()),
                connected_ae_ids: vec![1],
                permissions: WorkerPermissions::new(),
                ae_cursor: 0,
            },
        );
        Arc::new(RwLock::new(ZakoState {
            workers,
            sessions,
            worker_cursor: 0,
        }))
    }

    #[tokio::test]
    async fn reconcile_no_connected_aes_does_nothing() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });
        TlService::new(state_with_no_ae(), dispatcher).reconcile().await;
        assert_eq!(calls.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn reconcile_tl_restart_dangling_session() {
        let discord_session = session(1, 100);
        let state = state_with_connected_ae();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || {
                Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![discord_session]))
            }),
        });

        TlService::new(state, dispatcher).reconcile().await;

        let call_list = calls.lock().unwrap();
        assert_eq!(call_list.len(), 2);
        assert!(matches!(call_list[0], MockCall::FetchDiscordVoiceState));
        assert!(matches!(call_list[1], MockCall::Leave(s) if s == discord_session));
    }

    #[tokio::test]
    async fn reconcile_matching_sessions_no_leave() {
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![s]))),
        });

        TlService::new(state, dispatcher).reconcile().await;

        let call_list = calls.lock().unwrap();
        assert_eq!(call_list.len(), 1, "Should only call FetchDiscordVoiceState, no Leave");
        assert!(matches!(call_list[0], MockCall::FetchDiscordVoiceState));
    }

    #[tokio::test]
    async fn reconcile_partial_dangling() {
        let cached = session(1, 100);
        let dangling = session(2, 200);
        let state = state_with_session(cached);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || {
                Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![cached, dangling]))
            }),
        });

        TlService::new(state, dispatcher).reconcile().await;

        let call_list = calls.lock().unwrap();
        assert_eq!(call_list.len(), 2);
        assert!(matches!(call_list[0], MockCall::FetchDiscordVoiceState));
        assert!(matches!(call_list[1], MockCall::Leave(s) if s == dangling));
    }

    #[tokio::test]
    async fn reconcile_fetch_dispatch_error_continues() {
        let state = state_with_connected_ae();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Err(TlError::Transport("AE dead".into()))),
        });

        TlService::new(state, dispatcher).reconcile().await;
        // Should complete without panic
    }

    #[tokio::test]
    async fn reconcile_unexpected_response_continues() {
        let state = state_with_connected_ae();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });

        TlService::new(state, dispatcher).reconcile().await;
        // Should complete without panic
    }
}
