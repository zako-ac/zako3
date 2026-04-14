use std::sync::Arc;

use tl_protocol::{
    AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, SessionInfo, AudioEngineCommand,
};
use zako3_types::{GuildId, SessionState};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::{AeDispatcher, AeId, RouterError, RouterResult, SessionRoute, StateChangeEvent, ZakoState, router};

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
        let cmd_name = match &request.command {
            AudioEngineCommand::Join => "Join",
            AudioEngineCommand::SessionCommand(c) => match c {
                AudioEngineSessionCommand::Leave => "Leave",
                AudioEngineSessionCommand::GetSessionState => "GetSessionState",
                _ => "SessionCommand",
            },
            AudioEngineCommand::FetchDiscordVoiceState => "FetchDiscordVoiceState",
        };
        if let Some(session) = &request.session {
            info!(
                cmd = cmd_name,
                guild_id = ?session.guild_id,
                channel_id = ?session.channel_id,
                idempotency_key = ?request.idempotency_key,
                "execute: incoming command"
            );
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
                        info!(
                            guild_id = ?session_info.guild_id,
                            route_count = routes.len(),
                            "Leave: session not in TL cache, broadcasting to all known routes for guild"
                        );
                        for route in routes {
                            if let Err(e) = self.dispatcher.send(route, request.clone()).await {
                                warn!(
                                    worker_id = route.worker_id.0,
                                    ae_id = route.ae_id.0,
                                    error = %e,
                                    "broadcast Leave dispatch failed"
                                );
                            }
                        }
                    }
                    AudioEngineCommandResponse::Ok
                } else {
                    warn!(
                        cmd = cmd_name,
                        guild_id = ?request.session.map(|s| s.guild_id),
                        "routing failed: session not joined"
                    );
                    AudioEngineCommandResponse::Error(AudioEngineError::NotJoined)
                }
            }
            Err(RouterError::NoAvailableWorker) => {
                // No eligible worker via normal routing — try every known AE as a last resort.
                let all_routes: Vec<SessionRoute> = {
                    let state = self.state.read().await;
                    state
                        .workers
                        .iter()
                        .flat_map(|(worker_id, worker)| {
                            worker.connected_ae_ids.iter().map(|&ae_id| SessionRoute {
                                worker_id: *worker_id,
                                ae_id: AeId(ae_id),
                            })
                        })
                        .collect()
                };

                warn!(
                    guild_id = ?request.session.map(|s| s.guild_id),
                    fallback_route_count = all_routes.len(),
                    "no eligible worker for guild; trying all AEs as fallback"
                );

                for route in all_routes {
                    match self.dispatcher.send(route, request.clone()).await {
                        Ok(resp)
                            if matches!(
                                &resp,
                                AudioEngineCommandResponse::Ok
                                    | AudioEngineCommandResponse::Error(
                                        AudioEngineError::AlreadyJoined
                                    )
                            ) =>
                        {
                            info!(
                                worker_id = route.worker_id.0,
                                ae_id = route.ae_id.0,
                                already_joined = matches!(&resp, AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined)),
                                "fallback Join succeeded"
                            );
                            if let Some(session_info) = request.session {
                                let mut state = self.state.write().await;
                                state.sessions.insert(route, session_info);
                                info!(
                                    worker_id = route.worker_id.0,
                                    ae_id = route.ae_id.0,
                                    guild_id = ?session_info.guild_id,
                                    channel_id = ?session_info.channel_id,
                                    total_sessions = state.sessions.len(),
                                    "session committed (fallback)"
                                );
                            }
                            return AudioEngineCommandResponse::Ok;
                        }
                        Ok(err_resp) => {
                            warn!(
                                worker_id = route.worker_id.0,
                                ae_id = route.ae_id.0,
                                response = ?err_resp,
                                "AE rejected fallback Join, trying next"
                            );
                        }
                        Err(e) => {
                            warn!(
                                worker_id = route.worker_id.0,
                                ae_id = route.ae_id.0,
                                error = %e,
                                "dispatch error on fallback Join, trying next"
                            );
                        }
                    }
                }

                error!(
                    guild_id = ?request.session.map(|s| s.guild_id),
                    "all AEs failed on fallback Join"
                );
                AudioEngineCommandResponse::Error(AudioEngineError::InternalError(
                    "No available worker for this guild".into(),
                ))
            }
            Ok(RouterResult::Join(candidates)) => {
                info!(
                    candidate_count = candidates.len(),
                    guild_id = ?request.session.map(|s| s.guild_id),
                    "Join: trying {} candidate(s)", candidates.len()
                );
                for candidate in candidates {
                    info!(
                        worker_id = candidate.route.worker_id.0,
                        ae_id = candidate.route.ae_id.0,
                        guild_id = ?request.session.map(|s| s.guild_id),
                        "Join: dispatching to AE"
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
                            //
                            // Targeted write: only apply what this Join actually changes.
                            // Replacing the entire state with a pre-snapshot would silently
                            // erase concurrent Join commits for other guilds (TOCTOU).
                            let already_joined = matches!(&resp, AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined));
                            if let Some(session_info) = request.session {
                                let mut state = self.state.write().await;
                                state.sessions.insert(candidate.route, session_info);
                                // Carry over round-robin cursor advances from the routing decision.
                                state.worker_cursor = candidate.new_state_on_success.worker_cursor;
                                if let Some(worker) = state.workers.get_mut(&candidate.route.worker_id) {
                                    if let Some(new_worker) = candidate.new_state_on_success.workers.get(&candidate.route.worker_id) {
                                        worker.ae_cursor = new_worker.ae_cursor;
                                    }
                                }
                                info!(
                                    worker_id = candidate.route.worker_id.0,
                                    ae_id = candidate.route.ae_id.0,
                                    guild_id = ?session_info.guild_id,
                                    channel_id = ?session_info.channel_id,
                                    already_joined,
                                    total_sessions = state.sessions.len(),
                                    "session committed"
                                );
                            }
                            return AudioEngineCommandResponse::Ok;
                        }
                        Ok(err_resp) => {
                            warn!(
                                worker_id = candidate.route.worker_id.0,
                                ae_id = candidate.route.ae_id.0,
                                response = ?err_resp,
                                "Join failed on AE, trying next candidate"
                            );
                        }
                        Err(e) => {
                            warn!(
                                worker_id = candidate.route.worker_id.0,
                                ae_id = candidate.route.ae_id.0,
                                error = %e,
                                "dispatch error on Join, trying next candidate"
                            );
                        }
                    }
                }
                error!(
                    guild_id = ?request.session.map(|s| s.guild_id),
                    "all candidates failed to handle Join"
                );
                AudioEngineCommandResponse::Error(AudioEngineError::InternalError(
                    "All workers failed to handle Join".into(),
                ))
            }
            Ok(RouterResult::Session(success)) => {
                let route = success.route;
                info!(
                    cmd = cmd_name,
                    worker_id = route.worker_id.0,
                    ae_id = route.ae_id.0,
                    guild_id = ?request.session.map(|s| s.guild_id),
                    channel_id = ?request.session.map(|s| s.channel_id),
                    "session command: dispatching to AE"
                );

                match self.dispatcher.send(route, request).await {
                    Ok(response) => {
                        // SessionCommand routing never modifies state (new_state_on_success
                        // is always state.clone()), so no write-back needed. Performing a
                        // full state replacement here would cause the same TOCTOU overwrite
                        // of concurrent Join commits.
                        if matches!(response, AudioEngineCommandResponse::Error(_)) {
                            warn!(
                                cmd = cmd_name,
                                worker_id = route.worker_id.0,
                                ae_id = route.ae_id.0,
                                response = ?response,
                                "AE returned error for session command"
                            );
                        }
                        response
                    }
                    Err(e) => {
                        error!(
                            cmd = cmd_name,
                            worker_id = route.worker_id.0,
                            ae_id = route.ae_id.0,
                            error = %e,
                            "AE dispatch failed"
                        );
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
                    tracing::debug!(
                        guild_id = ?e.guild_id,
                        user_id = ?e.user_id,
                        "VoiceStateUpdate: bot joined/moved, no Leave needed"
                    );
                    return;
                }

                let (worker_id, sessions_to_leave) = {
                    let state = self.state.read().await;
                    let Some(worker_id) = state.worker_by_bot_client_id(&e.user_id) else {
                        tracing::debug!(
                            user_id = ?e.user_id,
                            guild_id = ?e.guild_id,
                            "VoiceStateUpdate: no worker found for user, ignoring"
                        );
                        return;
                    };
                    let sessions = state
                        .sessions_by_worker(worker_id)
                        .into_iter()
                        .filter(|(_, info)| info.guild_id == e.guild_id)
                        .collect::<Vec<_>>();
                    (worker_id, sessions)
                };

                info!(
                    worker_id = worker_id.0,
                    guild_id = ?e.guild_id,
                    session_count = sessions_to_leave.len(),
                    "VoiceStateUpdate: bot disconnected, triggering Leave for {} session(s)",
                    sessions_to_leave.len()
                );

                for (route, session_info) in sessions_to_leave {
                    info!(
                        worker_id = worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?e.guild_id,
                        channel_id = ?session_info.channel_id,
                        "auto-Leave: dispatching"
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
                        error!(
                            worker_id = worker_id.0,
                            ae_id = route.ae_id.0,
                            guild_id = ?session_info.guild_id,
                            error = %e,
                            "auto-Leave dispatch failed"
                        );
                    } else {
                        let mut state = self.state.write().await;
                        state.sessions.remove(&route);
                        info!(
                            worker_id = worker_id.0,
                            ae_id = route.ae_id.0,
                            guild_id = ?session_info.guild_id,
                            channel_id = ?session_info.channel_id,
                            total_sessions = state.sessions.len(),
                            "auto-Leave: session removed from state"
                        );
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

        info!(route_count = all_routes.len(), "reconcile: starting");

        // For each AE route, fetch Discord voice state and compare with cached sessions
        for route in &all_routes {
            let route = *route;
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

                    let discord_count = discord_sessions.len();
                    let dangling: Vec<SessionInfo> = discord_sessions
                        .into_iter()
                        .filter(|s| !cached.contains(s))
                        .collect();

                    info!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        discord_sessions = discord_count,
                        cached_sessions = cached.len(),
                        dangling_count = dangling.len(),
                        "reconcile: route checked"
                    );

                    // Find dangling sessions: in Discord but not in cache
                    for session_info in dangling {
                        info!(
                            worker_id = route.worker_id.0,
                            ae_id = route.ae_id.0,
                            guild_id = ?session_info.guild_id,
                            channel_id = ?session_info.channel_id,
                            "reconcile: leaving dangling session (in Discord but not cached)"
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
                            warn!(
                                worker_id = route.worker_id.0,
                                ae_id = route.ae_id.0,
                                guild_id = ?session_info.guild_id,
                                error = %e,
                                "reconcile: failed to leave dangling session"
                            );
                        }
                    }
                }
                Ok(other) => {
                    warn!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        response = ?other,
                        "reconcile: unexpected response from AE"
                    );
                }
                Err(e) => {
                    warn!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        error = %e,
                        "reconcile: dispatch failed"
                    );
                }
            }
        }

        info!(route_count = all_routes.len(), "reconcile: done");
    }

    /// Detects and evicts duplicate bots in the same channel.
    /// Multiple routes pointing to the same (guild_id, channel_id) can accumulate via
    /// TOCTOU races on concurrent Joins. Keeps one route per channel and sends Leave
    /// to the rest.
    /// Called from the periodic reconcile task (every 1 min).
    pub async fn evict_duplicates(&self) {
        let sessions: Vec<(SessionRoute, SessionInfo)> = {
            let state = self.state.read().await;
            state.sessions.iter().map(|(r, i)| (*r, *i)).collect()
        };

        // Group routes by channel — SessionInfo is (guild_id, channel_id)
        let mut by_channel: std::collections::HashMap<SessionInfo, Vec<SessionRoute>> =
            std::collections::HashMap::new();
        for (route, info) in &sessions {
            by_channel.entry(*info).or_default().push(*route);
        }

        let duplicates: Vec<(SessionRoute, SessionInfo)> = by_channel
            .into_iter()
            .filter(|(_, routes)| routes.len() > 1)
            .flat_map(|(info, routes)| {
                // Keep the first, evict the rest
                routes.into_iter().skip(1).map(move |route| (route, info))
            })
            .collect();

        if duplicates.is_empty() {
            tracing::debug!("evict_duplicates: no duplicates found");
            return;
        }

        warn!(
            count = duplicates.len(),
            "evict_duplicates: {} duplicate bot(s) in same channel — evicting",
            duplicates.len()
        );

        for (route, session_info) in duplicates {
            warn!(
                worker_id = route.worker_id.0,
                ae_id = route.ae_id.0,
                guild_id = ?session_info.guild_id,
                channel_id = ?session_info.channel_id,
                "evict_duplicates: sending Leave to duplicate bot"
            );
            let leave_req = AudioEngineCommandRequest {
                session: Some(session_info),
                command: AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
                headers: std::collections::HashMap::new(),
                idempotency_key: None,
            };
            if let Err(e) = self.dispatcher.send(route, leave_req).await {
                warn!(
                    worker_id = route.worker_id.0,
                    ae_id = route.ae_id.0,
                    error = %e,
                    "evict_duplicates: dispatch failed"
                );
            }
            let mut state = self.state.write().await;
            state.sessions.remove(&route);
        }

        info!("evict_duplicates: done");
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
            tracing::debug!("sync_sessions: no sessions to check");
            return;
        }

        info!(session_count = routes.len(), "sync_sessions: checking {} session(s)", routes.len());

        let mut to_remove = Vec::new();

        for (route, session_info) in &routes {
            let req = AudioEngineCommandRequest {
                session: Some(*session_info),
                command: AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
                headers: std::collections::HashMap::new(),
                idempotency_key: None,
            };

            // Use 5s timeout to avoid blocking on dead AEs
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                self.dispatcher.send(*route, req),
            )
            .await;

            match result {
                Ok(Ok(AudioEngineCommandResponse::SessionState(_))) => {
                    tracing::debug!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?session_info.guild_id,
                        "sync_sessions: session alive"
                    );
                }
                Ok(Ok(other)) => {
                    warn!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?session_info.guild_id,
                        response = ?other,
                        "sync_sessions: unexpected response, marking stale"
                    );
                    to_remove.push(*route);
                }
                Ok(Err(e)) => {
                    warn!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?session_info.guild_id,
                        error = %e,
                        "sync_sessions: dispatch failed, marking stale"
                    );
                    to_remove.push(*route);
                }
                Err(_timeout) => {
                    warn!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?session_info.guild_id,
                        "sync_sessions: timeout, marking stale"
                    );
                    to_remove.push(*route);
                }
            }
        }

        if !to_remove.is_empty() {
            info!(
                remove_count = to_remove.len(),
                "sync_sessions: removing {} stale session(s)", to_remove.len()
            );
            let mut state = self.state.write().await;
            for route in to_remove {
                state.sessions.remove(&route);
            }
            info!(remaining_sessions = state.sessions.len(), "sync_sessions: done");
        } else {
            info!(session_count = routes.len(), "sync_sessions: all sessions alive");
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

    fn two_routes_same_channel() -> Arc<RwLock<ZakoState>> {
        let s = session(1, 100);
        let route_a = SessionRoute { worker_id: WorkerId(0), ae_id: AeId(1) };
        let route_b = SessionRoute { worker_id: WorkerId(1), ae_id: AeId(1) };
        let mut sessions: FxHashMap<SessionRoute, SessionInfo> = Default::default();
        sessions.insert(route_a, s);
        sessions.insert(route_b, s);
        let mut workers = FxHashMap::default();
        for &wid in &[0u16, 1u16] {
            workers.insert(WorkerId(wid), Worker {
                worker_id: WorkerId(wid),
                bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
                discord_token: DiscordToken(String::new()),
                connected_ae_ids: vec![1],
                permissions: WorkerPermissions::new(),
                ae_cursor: 0,
            });
        }
        Arc::new(RwLock::new(ZakoState { workers, sessions, worker_cursor: 0 }))
    }

    #[tokio::test]
    async fn evict_duplicates_sends_leave_to_extra_bot() {
        let state = two_routes_same_channel();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        svc.evict_duplicates().await;

        let call_list = calls.lock().unwrap();
        // One Leave should have been sent for the duplicate
        let leaves: Vec<_> = call_list.iter().filter(|c| matches!(c, MockCall::Leave(_))).collect();
        assert_eq!(leaves.len(), 1, "exactly one duplicate should be evicted");

        // State should now have one session remaining
        let remaining = state.read().await.sessions.len();
        assert_eq!(remaining, 1, "one session should remain after eviction");
    }

    #[tokio::test]
    async fn evict_duplicates_no_duplicates_does_nothing() {
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });

        TlService::new(state, dispatcher).evict_duplicates().await;

        assert_eq!(calls.lock().unwrap().len(), 0, "no calls when no duplicates");
    }
}
