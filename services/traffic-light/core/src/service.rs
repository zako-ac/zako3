use std::sync::{Arc, Mutex};

use opentelemetry::global;
use rustc_hash::FxHashMap;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, SessionInfo,
};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_types::{ChannelId, GuildId, SessionState};

use crate::{
    router, AeDispatcher, RouterError, RouterResult, SessionRoute, StateChangeEvent,
    ZakoState,
};

/// Number of consecutive negative observations required before TL tears down or leaves a
/// session. Sync/reconcile run every 60s, so N=3 ≈ 3 minutes of sustained failure before a
/// bot is kicked. This prevents a single transient blip (RPC timeout, voice-server reconnect)
/// from evicting a healthy session and triggering a leave/rejoin.
const TEARDOWN_THRESHOLD: u8 = 3;

/// Number of consecutive reconcile cycles a cached session must be absent from the AE's live
/// Discord voice state before TL prunes it. Reconcile runs every 60s, so N=2 ≈ 2 minutes of
/// sustained absence. The AE's voice-state report is authoritative (it reads the AE's own
/// Discord gateway), so once a session has been absent across several cycles it is definitively
/// stale. The grace guards the narrow window between a Join being accepted by the AE (returns
/// Ok) and the bot's voice connection becoming visible in the voice-state report — a fresh join
/// must not be spuriously evicted.
const RECONCILE_ABSENCE_THRESHOLD: u8 = 2;

pub struct TlService {
    state: Arc<RwLock<ZakoState>>,
    dispatcher: Arc<dyn AeDispatcher>,
    /// Consecutive `sync_sessions` failures per session. A single bot can serve many guilds
    /// at once, so strikes are tracked per `(route, guild)` — one guild's repeated failures
    /// must not evict a healthy session the same bot holds in another guild.
    sync_failures: Mutex<FxHashMap<(SessionRoute, GuildId), u8>>,
    /// Consecutive reconcile cycles a cached session has been absent from the AE's live Discord
    /// voice state, per `(route, guild)`. Unlike `sync_failures` (which probes via
    /// GetSessionState and can be fooled by a surviving stale AE SessionControl), this tracks
    /// absence from the AE's authoritative voice-state report, so a stale cache entry that
    /// `sync_sessions` would otherwise never evict still gets pruned.
    reconcile_absent: Mutex<FxHashMap<(SessionRoute, GuildId), u8>>,
}

impl TlService {
    pub fn new(state: Arc<RwLock<ZakoState>>, dispatcher: Arc<dyn AeDispatcher>) -> Self {
        Self {
            state,
            dispatcher,
            sync_failures: Mutex::new(FxHashMap::default()),
            reconcile_absent: Mutex::new(FxHashMap::default()),
        }
    }

    pub fn state(&self) -> Arc<RwLock<ZakoState>> {
        self.state.clone()
    }

    pub async fn execute(&self, request: AudioEngineCommandRequest) -> AudioEngineCommandResponse {
        let cmd_name = request.command.operation();

        let parent_cx = global::get_text_map_propagator(|p| p.extract(&request.headers));
        let span = tracing::info_span!("tl.execute", otel.name = %format!("tl.{cmd_name}"), cmd = cmd_name);
        let _ = span.set_parent(parent_cx);
        let _span_guard = span.enter();

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
                // Deterministic routing already considered every eligible worker (the HRW
                // ranking walks the whole set). A genuine NoAvailableWorker therefore means
                // no worker is eligible for this guild — there is no safe fallback. The old
                // "try every AE as a last resort" branch is intentionally gone: spraying a
                // Join across unrelated bots is exactly what let a second physical bot join
                // one channel.
                warn!(
                    guild_id = ?request.session.map(|s| s.guild_id),
                    "no eligible worker for guild; rejecting Join"
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
                        Ok(resp)
                            if matches!(
                                &resp,
                                AudioEngineCommandResponse::Ok
                                    | AudioEngineCommandResponse::Error(
                                        AudioEngineError::AlreadyJoined
                                    )
                            ) =>
                        {
                            // AlreadyJoined from AE means the session is active there —
                            // treat as success (covers state-drift resync joins).
                            //
                            // Targeted write: only apply what this Join actually changes.
                            // Replacing the entire state with a pre-snapshot would silently
                            // erase concurrent Join commits for other guilds (TOCTOU).
                            let already_joined = matches!(
                                &resp,
                                AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined)
                            );
                            if let Some(session_info) = request.session {
                                let mut state = self.state.write().await;
                                state.insert_session(candidate.route, session_info);
                                info!(
                                    worker_id = candidate.route.worker_id.0,
                                    ae_id = candidate.route.ae_id.0,
                                    guild_id = ?session_info.guild_id,
                                    channel_id = ?session_info.channel_id,
                                    already_joined,
                                    total_sessions = state.total_session_count(),
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
                let is_leave = matches!(
                    request.command,
                    AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave)
                );
                info!(
                    cmd = cmd_name,
                    worker_id = route.worker_id.0,
                    ae_id = route.ae_id.0,
                    guild_id = ?request.session.map(|s| s.guild_id),
                    channel_id = ?request.session.map(|s| s.channel_id),
                    "session command: dispatching to AE"
                );

                let session_guild = request.session.map(|s| s.guild_id);
                match self.dispatcher.send(route, request).await {
                    Ok(response) => {
                        // SessionCommand routing never allocates a route, so no write-back is
                        // needed on success. But two outcomes leave the cached route orphaned,
                        // which `sync_sessions` would
                        // otherwise flag as NotJoined for ~3 minutes before pruning:
                        //   1. A successful Leave — the AE dropped the session, so must TL.
                        //   2. A NotJoined error — the AE has no such session; the cache is
                        //      stale (e.g. the bot was kicked / left out-of-band).
                        // Evict the route immediately in both cases. Targeted remove only —
                        // a full state replacement would TOCTOU-clobber concurrent Join commits.
                        let evict = (is_leave
                            && matches!(response, AudioEngineCommandResponse::Ok))
                            || matches!(
                                response,
                                AudioEngineCommandResponse::Error(AudioEngineError::NotJoined)
                            );
                        if evict {
                            let mut state = self.state.write().await;
                            if let Some(guild_id) = session_guild {
                                if state.remove_session(&route, guild_id) {
                                    info!(
                                        cmd = cmd_name,
                                        worker_id = route.worker_id.0,
                                        ae_id = route.ae_id.0,
                                        guild_id = ?guild_id,
                                        total_sessions = state.total_session_count(),
                                        "session evicted from cache after session command"
                                    );
                                }
                                self.sync_failures.lock().unwrap().remove(&(route, guild_id));
                            }
                        }
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
                        state.remove_session(&route, session_info.guild_id);
                        info!(
                            worker_id = worker_id.0,
                            ae_id = route.ae_id.0,
                            guild_id = ?session_info.guild_id,
                            channel_id = ?session_info.channel_id,
                            total_sessions = state.total_session_count(),
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

    pub async fn list_bot_ids(&self) -> Vec<String> {
        let state = self.state.read().await;
        state
            .workers
            .values()
            .map(|w| w.bot_client_id.0.clone())
            .filter(|id| !id.is_empty())
            .collect()
    }

    pub async fn report_guilds(&self, token: String, guilds: Vec<GuildId>) {
        let mut state = self.state.write().await;
        if let Some(worker) = state
            .workers
            .values_mut()
            .find(|w| w.discord_token.0 == token)
        {
            info!(
                guild_count = guilds.len(),
                "Updating worker guild permissions"
            );
            worker.permissions.set_allowed_guilds(guilds);
        } else {
            warn!(token, "report_guilds: no worker found for token");
        }
    }

    /// Reconciles TL's session cache toward the AEs' live Discord connections. For each AE
    /// route it fetches the actual voice connections and *re-adopts* any the cache is missing
    /// (e.g. after a TL restart, or an AE restart that rejoined from persisted state). TL
    /// trusts the AE's live connections as truth rather than kicking sessions it doesn't
    /// recognise — this is what previously caused the leave/rejoin flapping. Sessions the AE
    /// is no longer serving are pruned separately by `sync_sessions`.
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

        // For each AE route, fetch Discord voice state and reconcile with cached sessions.
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
                    // Sessions TL already tracks for this route (one bot in possibly many guilds).
                    let cached: Vec<SessionInfo> = {
                        let state = self.state.read().await;
                        state.sessions_for_route(&route)
                    };
                    // The AE's live Discord channel per guild.
                    let live_by_guild: FxHashMap<GuildId, ChannelId> = discord_sessions
                        .iter()
                        .map(|s| (s.guild_id, s.channel_id))
                        .collect();

                    let discord_count = discord_sessions.len();
                    let adopt_count = discord_sessions
                        .iter()
                        .filter(|live| !cached.iter().any(|c| c.guild_id == live.guild_id))
                        .count();

                    info!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        discord_sessions = discord_count,
                        cached_sessions = cached.len(),
                        adopt_count,
                        "reconcile: route checked"
                    );

                    // Re-adopt / repair toward the AE's live Discord connections, which TL
                    // trusts as the source of truth (never kicking a bot it doesn't recognise —
                    // that caused the old leave/rejoin flapping). A bot can legitimately serve
                    // many guilds at once, so:
                    //   - every live session the cache is missing is adopted (not just one);
                    //   - a cached session whose channel differs from the AE's report is
                    //     repaired to the live channel (covers a bot moved between channels,
                    //     where the AE's old SessionControl would otherwise survive and leave
                    //     a stale entry that `sync_sessions` never evicts).
                    // Sessions the AE no longer reports at all are left to `sync_sessions`'
                    // 3-strike eviction, which protects healthy-but-busy bots from transient
                    // dispatch blips.
                    let mut repaired = 0u32;
                    {
                        let mut state = self.state.write().await;
                        for live in &discord_sessions {
                            if !cached.iter().any(|c| c.guild_id == live.guild_id) {
                                info!(
                                    worker_id = route.worker_id.0,
                                    ae_id = route.ae_id.0,
                                    guild_id = ?live.guild_id,
                                    channel_id = ?live.channel_id,
                                    "reconcile: re-adopting live session into TL cache"
                                );
                                state.insert_session(route, *live);
                            }
                        }
                        for cached_info in &cached {
                            if let Some(&live_channel) = live_by_guild.get(&cached_info.guild_id) {
                                if live_channel != cached_info.channel_id {
                                    info!(
                                        worker_id = route.worker_id.0,
                                        ae_id = route.ae_id.0,
                                        guild_id = ?cached_info.guild_id,
                                        old_channel = ?cached_info.channel_id,
                                        new_channel = ?live_channel,
                                        "reconcile: repairing session channel (bot moved)"
                                    );
                                    state.insert_session(
                                        route,
                                        SessionInfo {
                                            guild_id: cached_info.guild_id,
                                            channel_id: live_channel,
                                        },
                                    );
                                    repaired += 1;
                                }
                            }
                        }
                    }
                    if repaired > 0 {
                        info!(
                            worker_id = route.worker_id.0,
                            ae_id = route.ae_id.0,
                            repaired,
                            "reconcile: repaired moved session(s)"
                        );
                    }

                    // Prune cached sessions the AE no longer reports as live Discord connections.
                    // The AE's voice-state report is authoritative (it reads the AE's own Discord
                    // gateway), so a cached session whose guild is absent is stale — typically a
                    // bot that left or moved out-of-band while the AE's old SessionControl
                    // survived. `sync_sessions` would never evict it (GetSessionState keeps
                    // succeeding on the stale SessionControl), leaving the bot permanently
                    // "already joined" in a guild it isn't actually in, so Join there becomes a
                    // silent no-op. Require a few consecutive absences so an in-flight Join
                    // (accepted by the AE but whose voice connection isn't visible yet) isn't
                    // spuriously evicted — the next reconcile re-adopts it once it appears.
                    let mut pruned = 0u32;
                    {
                        let mut state = self.state.write().await;
                        let mut absent = self.reconcile_absent.lock().unwrap();
                        for cached_info in &cached {
                            if live_by_guild.contains_key(&cached_info.guild_id) {
                                absent.remove(&(route, cached_info.guild_id));
                                continue;
                            }
                            let n = {
                                let e = absent.entry((route, cached_info.guild_id)).or_insert(0);
                                *e = e.saturating_add(1);
                                *e
                            };
                            if n >= RECONCILE_ABSENCE_THRESHOLD {
                                if state.remove_session(&route, cached_info.guild_id) {
                                    info!(
                                        worker_id = route.worker_id.0,
                                        ae_id = route.ae_id.0,
                                        guild_id = ?cached_info.guild_id,
                                        "reconcile: pruned stale session absent from AE live voice state"
                                    );
                                    pruned += 1;
                                }
                                absent.remove(&(route, cached_info.guild_id));
                            }
                        }
                        // Drop counters for sessions no longer tracked at all so the map can't
                        // grow without bound.
                        let tracked: Vec<(SessionRoute, GuildId)> =
                            cached.iter().map(|c| (route, c.guild_id)).collect();
                        absent.retain(|(r, g), _| tracked.contains(&(*r, *g)));
                    }
                    if pruned > 0 {
                        info!(
                            worker_id = route.worker_id.0,
                            ae_id = route.ae_id.0,
                            pruned,
                            "reconcile: pruned stale session(s)"
                        );
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
            state.all_sessions()
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
            state.remove_session(&route, session_info.guild_id);
        }

        info!("evict_duplicates: done");
    }

    /// Fetches current session state from all connected AEs and removes stale sessions.
    /// Called periodically (every 1 min) and on command failures to reconcile state drift.
    pub async fn sync_sessions(&self) {
        // Snapshot all current sessions across every route.
        let routes: Vec<(SessionRoute, SessionInfo)> = {
            let state = self.state.read().await;
            state.all_sessions()
        };

        if routes.is_empty() {
            tracing::debug!("sync_sessions: no sessions to check");
            return;
        }

        info!(
            session_count = routes.len(),
            "sync_sessions: checking {} session(s)",
            routes.len()
        );

        let mut to_remove = Vec::new();

        for (route, session_info) in &routes {
            let req = AudioEngineCommandRequest {
                session: Some(*session_info),
                command: AudioEngineCommand::SessionCommand(
                    AudioEngineSessionCommand::GetSessionState,
                ),
                headers: std::collections::HashMap::new(),
                idempotency_key: None,
            };

            // Use 5s timeout to avoid blocking on dead AEs
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                self.dispatcher.send(*route, req),
            )
            .await;

            let outcome: Result<(), String> = match result {
                Ok(Ok(AudioEngineCommandResponse::SessionState(_))) => Ok(()),
                Ok(Ok(other)) => Err(format!("unexpected response: {other:?}")),
                Ok(Err(e)) => Err(format!("dispatch failed: {e}")),
                Err(_timeout) => Err("timeout".to_string()),
            };

            // Strikes are tracked per (route, guild) so a bot serving several guilds gets an
            // independent health signal per guild.
            let strike_key = (*route, session_info.guild_id);
            match outcome {
                Ok(()) => {
                    // Healthy — clear any accumulated strikes so a future blip starts fresh.
                    self.sync_failures.lock().unwrap().remove(&strike_key);
                    tracing::debug!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?session_info.guild_id,
                        "sync_sessions: session alive"
                    );
                }
                Err(reason) => {
                    // Negative result. Require TEARDOWN_THRESHOLD consecutive failures before
                    // removing the session — a single transient blip must not evict a live bot.
                    let strikes = {
                        let mut m = self.sync_failures.lock().unwrap();
                        let n = m.entry(strike_key).or_insert(0);
                        *n = n.saturating_add(1);
                        *n
                    };
                    warn!(
                        worker_id = route.worker_id.0,
                        ae_id = route.ae_id.0,
                        guild_id = ?session_info.guild_id,
                        reason = %reason,
                        strikes,
                        threshold = TEARDOWN_THRESHOLD,
                        "sync_sessions: check failed"
                    );
                    if strikes >= TEARDOWN_THRESHOLD {
                        to_remove.push(strike_key);
                    }
                }
            }
        }

        if !to_remove.is_empty() {
            info!(
                remove_count = to_remove.len(),
                "sync_sessions: removing {} stale session(s)",
                to_remove.len()
            );
            let mut state = self.state.write().await;
            let mut strikes = self.sync_failures.lock().unwrap();
            for (route, guild_id) in &to_remove {
                state.remove_session(route, *guild_id);
                strikes.remove(&(*route, *guild_id));
            }
            info!(
                remaining_sessions = state.total_session_count(),
                "sync_sessions: done"
            );
        } else {
            info!(
                session_count = routes.len(),
                "sync_sessions: all sessions alive"
            );
        }

        // Prune strike counters for sessions that are no longer tracked at all (e.g. left
        // cleanly), so the map can't grow without bound.
        self.sync_failures
            .lock()
            .unwrap()
            .retain(|(r, g), _| routes.iter().any(|(rr, info)| rr == r && info.guild_id == *g));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AeId, DiscordToken, TlError, Worker, WorkerId, WorkerPermissions};
    use rustc_hash::FxHashMap;
    use std::sync::Mutex;
    use tl_protocol::{AudioEngineCommand, AudioEngineSessionCommand, SessionInfo};
    use zako3_types::{ChannelId, GuildId};

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
                    self.calls
                        .lock()
                        .unwrap()
                        .push(MockCall::FetchDiscordVoiceState);
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
            },
        );
        Arc::new(RwLock::new(ZakoState {
            workers,
            sessions: Default::default(),
        }))
    }

    fn state_with_session(s: SessionInfo) -> Arc<RwLock<ZakoState>> {
        let mut sessions: FxHashMap<SessionRoute, FxHashMap<GuildId, SessionInfo>> =
            Default::default();
        let mut by_guild: FxHashMap<GuildId, SessionInfo> = Default::default();
        by_guild.insert(s.guild_id, s);
        sessions.insert(route(), by_guild);
        let mut workers = FxHashMap::default();
        workers.insert(
            WorkerId(0),
            Worker {
                worker_id: WorkerId(0),
                bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
                discord_token: DiscordToken(String::new()),
                connected_ae_ids: vec![1],
                permissions: WorkerPermissions::new(),
            },
        );
        Arc::new(RwLock::new(ZakoState {
            workers,
            sessions,
        }))
    }

    #[tokio::test]
    async fn reconcile_no_connected_aes_does_nothing() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });
        TlService::new(state_with_no_ae(), dispatcher)
            .reconcile()
            .await;
        assert_eq!(calls.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn reconcile_readopts_uncached_live_session() {
        // AE is connected to a session TL has no cache entry for (e.g. after a TL restart, or
        // an AE that rejoined from persisted state). TL must re-adopt it, never leave it.
        let discord_session = session(1, 100);
        let state = state_with_connected_ae();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || {
                Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![
                    discord_session,
                ]))
            }),
        });

        TlService::new(state.clone(), dispatcher).reconcile().await;

        // No Leave was ever sent.
        let call_list = calls.lock().unwrap();
        assert!(
            call_list.iter().all(|c| matches!(c, MockCall::FetchDiscordVoiceState)),
            "reconcile must re-adopt, never Leave"
        );
        drop(call_list);

        // The live session is now tracked under the route.
        let s = state.read().await;
        assert!(
            s.sessions_for_route(&route()).contains(&discord_session),
            "live session must be tracked under the route"
        );
    }

    #[tokio::test]
    async fn reconcile_adopts_all_live_sessions_on_one_route() {
        // The reported bug: a single bot serves two guilds at once. The old one-session-per-route
        // model could track only one, so the second guild's session was dropped every cycle and
        // the bot was invisible to HQ in that guild. Reconcile must adopt BOTH live sessions
        // (and send no Leave).
        let cached = session(1, 100);
        let extra = session(2, 200);
        let state = state_with_session(cached);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || {
                Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![
                    cached, extra,
                ]))
            }),
        });

        TlService::new(state.clone(), dispatcher).reconcile().await;

        let call_list = calls.lock().unwrap();
        assert!(
            call_list.iter().all(|c| matches!(c, MockCall::FetchDiscordVoiceState)),
            "no Leave during reconcile"
        );
        drop(call_list);
        // Both the originally-cached and the second guild's session are now tracked.
        let s = state.read().await;
        let tracked = s.sessions_for_route(&route());
        assert!(tracked.contains(&cached), "cached session preserved");
        assert!(tracked.contains(&extra), "second guild's session adopted");
    }

    #[tokio::test]
    async fn reconcile_repairs_cached_channel_on_move() {
        // A bot moved to a different channel in the same guild. The AE reports the new channel;
        // the cache still holds the old one (whose SessionControl survives a move, so
        // `sync_sessions` would never evict it). Reconcile must repair the cached channel.
        let old = session(1, 100);
        let new = session(1, 200);
        let state = state_with_session(old);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || {
                Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![new]))
            }),
        });

        TlService::new(state.clone(), dispatcher).reconcile().await;

        let call_list = calls.lock().unwrap();
        assert!(
            call_list.iter().all(|c| matches!(c, MockCall::FetchDiscordVoiceState)),
            "no Leave during reconcile"
        );
        drop(call_list);
        let s = state.read().await;
        let tracked = s.sessions_for_route(&route());
        assert!(tracked.contains(&new), "cached channel repaired to the live channel");
        assert!(!tracked.contains(&old), "stale channel entry gone");
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
        assert_eq!(
            call_list.len(),
            1,
            "Should only call FetchDiscordVoiceState, no Leave"
        );
        assert!(matches!(call_list[0], MockCall::FetchDiscordVoiceState));
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

    #[tokio::test]
    async fn reconcile_prunes_cached_session_absent_from_live() {
        // A cached session whose guild the AE no longer reports as a live Discord connection is
        // stale (the bot left/moved out-of-band, but the AE's old SessionControl survived, so
        // `sync_sessions`' GetSessionState would keep succeeding and never evict it). Without a
        // fix, TL keeps answering "already joined" in that guild and Join there is a permanent
        // no-op. Reconcile must prune it. The absence must persist across
        // RECONCILE_ABSENCE_THRESHOLD cycles so an in-flight Join isn't spuriously evicted.
        let stale = session(1, 100);
        let state = state_with_session(stale);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![]))),
        });
        let service = TlService::new(state.clone(), dispatcher);

        // A single absent cycle is not enough (guards the in-flight Join window).
        service.reconcile().await;
        assert!(
            state
                .read()
                .await
                .sessions_for_route(&route())
                .contains(&stale),
            "must not prune after a single absent cycle"
        );

        // Sustained absence across the threshold prunes the stale session.
        for _ in 0..RECONCILE_ABSENCE_THRESHOLD {
            service.reconcile().await;
        }
        assert!(
            state.read().await.sessions_for_route(&route()).is_empty(),
            "stale session pruned after sustained absence"
        );

        // Reconcile never sends Leave — pruning only drops the cache entry.
        let call_list = calls.lock().unwrap();
        assert!(
            call_list
                .iter()
                .all(|c| matches!(c, MockCall::FetchDiscordVoiceState)),
            "no Leave during reconcile"
        );
    }

    #[tokio::test]
    async fn sync_sessions_keeps_session_on_single_failure() {
        // One bad GetSessionState response must NOT evict a cached session.
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Error(AudioEngineError::NotJoined))),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        svc.sync_sessions().await;

        assert_eq!(
            state.read().await.total_session_count(),
            1,
            "a single NotJoined must not remove the session"
        );
    }

    #[tokio::test]
    async fn sync_sessions_evicts_after_threshold() {
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Error(AudioEngineError::NotJoined))),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        for i in 0..TEARDOWN_THRESHOLD {
            svc.sync_sessions().await;
            let remaining = state.read().await.total_session_count();
            if i + 1 < TEARDOWN_THRESHOLD {
                assert_eq!(remaining, 1, "session kept before threshold (pass {})", i + 1);
            } else {
                assert_eq!(remaining, 0, "session evicted on the threshold pass");
            }
        }
    }

    #[tokio::test]
    async fn sync_sessions_failure_strike_resets_on_recovery() {
        // Fail twice (below threshold of 3), then succeed → strike resets, session survives
        // a subsequent failure.
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(move || {
                let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // calls 0,1 fail; call 2 succeeds (reset); call 3 fails again.
                if n == 2 {
                    Ok(AudioEngineCommandResponse::SessionState(SessionState {
                        guild_id: GuildId::from(1u64),
                        channel_id: ChannelId::from(100u64),
                        queues: Default::default(),
                    }))
                } else {
                    Ok(AudioEngineCommandResponse::Error(AudioEngineError::NotJoined))
                }
            }),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        for _ in 0..4 {
            svc.sync_sessions().await;
        }
        assert_eq!(
            state.read().await.total_session_count(),
            1,
            "interleaved success resets strikes, so no eviction"
        );
    }

    fn leave_request(s: SessionInfo) -> AudioEngineCommandRequest {
        AudioEngineCommandRequest {
            session: Some(s),
            command: AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
            headers: std::collections::HashMap::new(),
            idempotency_key: None,
        }
    }

    #[tokio::test]
    async fn execute_leave_evicts_cached_session() {
        // A successful Leave must remove the route from TL's cache immediately, so
        // sync_sessions never flags the now-departed bot as NotJoined.
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        let resp = svc.execute(leave_request(s)).await;

        assert!(matches!(resp, AudioEngineCommandResponse::Ok));
        assert_eq!(
            state.read().await.total_session_count(),
            0,
            "successful Leave must evict the cached session"
        );
    }

    #[tokio::test]
    async fn execute_session_command_notjoined_evicts_stale_session() {
        // The AE has no such session (stale cache, e.g. bot kicked out-of-band). A NotJoined
        // response to a session command must prune the orphaned route, not wait 3 sync strikes.
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| {
                Ok(AudioEngineCommandResponse::Error(AudioEngineError::NotJoined))
            }),
        });

        let req = AudioEngineCommandRequest {
            session: Some(s),
            command: AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
            headers: std::collections::HashMap::new(),
            idempotency_key: None,
        };
        let svc = TlService::new(state.clone(), dispatcher);
        let resp = svc.execute(req).await;

        assert!(matches!(
            resp,
            AudioEngineCommandResponse::Error(AudioEngineError::NotJoined)
        ));
        assert_eq!(
            state.read().await.total_session_count(),
            0,
            "NotJoined for a session command must evict the stale route"
        );
    }

    #[tokio::test]
    async fn execute_non_leave_success_keeps_session() {
        // A successful non-Leave session command must NOT evict — the bot is still serving.
        let s = session(1, 100);
        let state = state_with_session(s);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });

        let req = AudioEngineCommandRequest {
            session: Some(s),
            command: AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::NextMusic),
            headers: std::collections::HashMap::new(),
            idempotency_key: None,
        };
        let svc = TlService::new(state.clone(), dispatcher);
        let _ = svc.execute(req).await;

        assert_eq!(
            state.read().await.total_session_count(),
            1,
            "a successful non-Leave command must keep the session"
        );
    }

    fn two_routes_same_channel() -> Arc<RwLock<ZakoState>> {
        let s = session(1, 100);
        let route_a = SessionRoute {
            worker_id: WorkerId(0),
            ae_id: AeId(1),
        };
        let route_b = SessionRoute {
            worker_id: WorkerId(1),
            ae_id: AeId(1),
        };
        let mut sessions: FxHashMap<SessionRoute, FxHashMap<GuildId, SessionInfo>> =
            Default::default();
        let mut by_guild_a: FxHashMap<GuildId, SessionInfo> = Default::default();
        by_guild_a.insert(s.guild_id, s);
        sessions.insert(route_a, by_guild_a);
        let mut by_guild_b: FxHashMap<GuildId, SessionInfo> = Default::default();
        by_guild_b.insert(s.guild_id, s);
        sessions.insert(route_b, by_guild_b);
        let mut workers = FxHashMap::default();
        for &wid in &[0u16, 1u16] {
            workers.insert(
                WorkerId(wid),
                Worker {
                    worker_id: WorkerId(wid),
                    bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
                    discord_token: DiscordToken(String::new()),
                    connected_ae_ids: vec![1],
                    permissions: WorkerPermissions::new(),
                },
            );
        }
        Arc::new(RwLock::new(ZakoState {
            workers,
            sessions,
        }))
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
        // One Leave should have been sent, targeting the duplicated (guild, channel).
        let leaves: Vec<SessionInfo> = call_list
            .iter()
            .filter_map(|c| match c {
                MockCall::Leave(s) => Some(*s),
                _ => None,
            })
            .collect();
        assert_eq!(leaves.len(), 1, "exactly one duplicate should be evicted");
        assert_eq!(leaves[0], session(1, 100), "the duplicate session is the one left");

        // State should now have one session remaining
        let remaining = state.read().await.total_session_count();
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

        assert_eq!(
            calls.lock().unwrap().len(),
            0,
            "no calls when no duplicates"
        );
    }

    fn state_with_permitted_workers(n: u16, guild: u64) -> Arc<RwLock<ZakoState>> {
        let mut workers = FxHashMap::default();
        for id in 0..n {
            let permissions = WorkerPermissions::new();
            permissions.add_allowed_guild(GuildId::from(guild));
            workers.insert(
                WorkerId(id),
                Worker {
                    worker_id: WorkerId(id),
                    bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
                    discord_token: DiscordToken(String::new()),
                    connected_ae_ids: vec![1],
                    permissions,
                },
            );
        }
        Arc::new(RwLock::new(ZakoState {
            workers,
            sessions: Default::default(),
        }))
    }

    fn join_request(s: SessionInfo) -> AudioEngineCommandRequest {
        AudioEngineCommandRequest {
            session: Some(s),
            command: AudioEngineCommand::Join,
            headers: std::collections::HashMap::new(),
            idempotency_key: None,
        }
    }

    #[tokio::test]
    async fn execute_join_already_joined_commits_single_route() {
        // The AE reports AlreadyJoined (the bot is already in the channel). TL must treat that
        // as success, commit exactly one route, and a second identical Join must not allocate a
        // new worker — no second physical bot can appear.
        let s = session(1, 100);
        let state = state_with_permitted_workers(4, 1);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| {
                Ok(AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined))
            }),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        let r1 = svc.execute(join_request(s)).await;
        assert!(matches!(r1, AudioEngineCommandResponse::Ok));
        let route_after_first = {
            let st = state.read().await;
            assert_eq!(st.total_session_count(), 1, "exactly one route committed");
            *st.sessions.keys().next().unwrap()
        };

        let r2 = svc.execute(join_request(s)).await;
        assert!(matches!(r2, AudioEngineCommandResponse::Ok));
        let st = state.read().await;
        assert_eq!(st.total_session_count(), 1, "re-Join must not add a second route");
        assert!(
            st.sessions.contains_key(&route_after_first),
            "re-Join must resolve to the same route (idempotent)"
        );
    }

    #[tokio::test]
    async fn execute_join_no_permitted_worker_is_rejected_without_fallback() {
        // No worker permitted for the guild. With the all-AE fallback removed, this must be a
        // clean error — never a spray of Joins across unrelated bots.
        let s = session(1, 100);
        let state = state_with_connected_ae(); // worker exists but has no guild permission
        let calls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = Arc::new(TestDispatcher {
            calls: calls.clone(),
            response: Arc::new(|| Ok(AudioEngineCommandResponse::Ok)),
        });

        let svc = TlService::new(state.clone(), dispatcher);
        let resp = svc.execute(join_request(s)).await;

        assert!(matches!(
            resp,
            AudioEngineCommandResponse::Error(AudioEngineError::InternalError(_))
        ));
        assert_eq!(
            state.read().await.total_session_count(),
            0,
            "rejected Join must not commit any session"
        );
    }
}
