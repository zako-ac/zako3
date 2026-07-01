use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;
use thiserror::Error;
use tl_protocol::SessionInfo;
use zako3_types::GuildId;

use crate::{
    AeId, AudioEngineCommand, AudioEngineCommandRequest, SessionRoute, Worker, WorkerId, ZakoState,
};

#[derive(Debug, Clone, Error)]
pub enum RouterError {
    #[error("Not joined the voice channel")]
    NotJoined,

    #[error("No available worker for this guild")]
    NoAvailableWorker,
}

#[derive(Debug, Clone)]
pub struct RouterSuccess {
    pub route: SessionRoute,
}

/// Result of a routing decision.
pub enum RouterResult {
    /// Join candidates in deterministic priority order. TlService tries them sequentially,
    /// stopping at the first that accepts (or reports AlreadyJoined).
    Join(Vec<RouterSuccess>),
    /// The single route for a SessionCommand.
    Session(RouterSuccess),
}

pub fn route(
    state: &ZakoState,
    request: &AudioEngineCommandRequest,
) -> Result<RouterResult, RouterError> {
    let session = request.session.expect("session required for routing");
    match request.command {
        AudioEngineCommand::Join => {
            let ranked = rank_workers_for_session(state, &session);

            // If the session is already tracked, that route is the real bot in the channel —
            // put it first so a re-Join is idempotent (the AE answers AlreadyJoined). The
            // deterministic ranking follows as failover in case that AE actually lost the
            // session (state-drift recovery after an AE restart).
            let existing_route = state.session_by_info(&session);
            if let Some(route) = existing_route {
                tracing::info!(
                    worker_id = route.worker_id.0,
                    ae_id = route.ae_id.0,
                    guild_id = ?session.guild_id,
                    channel_id = ?session.channel_id,
                    "router: Join for already-tracked session, reusing existing route first (idempotent)"
                );
            }

            let mut candidates: Vec<RouterSuccess> = Vec::new();
            if let Some(route) = existing_route {
                candidates.push(RouterSuccess { route });
            }
            for worker in &ranked {
                let route = SessionRoute {
                    worker_id: worker.worker_id,
                    ae_id: AeId(ae_id_for(worker)),
                };
                if Some(route) == existing_route {
                    continue;
                }
                candidates.push(RouterSuccess { route });
            }

            if candidates.is_empty() {
                tracing::warn!(
                    guild_id = ?session.guild_id,
                    total_workers = state.workers.len(),
                    "router: no eligible worker for Join (no access or no connected AEs)"
                );
                return Err(RouterError::NoAvailableWorker);
            }

            tracing::info!(
                guild_id = ?session.guild_id,
                channel_id = ?session.channel_id,
                candidate_count = candidates.len(),
                first_candidate_worker_id = candidates[0].route.worker_id.0,
                "router: built deterministic Join candidates"
            );

            Ok(RouterResult::Join(candidates))
        }
        AudioEngineCommand::SessionCommand(ref cmd) => {
            if let Some(route) = state.session_by_info(&session) {
                tracing::debug!(
                    worker_id = route.worker_id.0,
                    ae_id = route.ae_id.0,
                    guild_id = ?session.guild_id,
                    cmd = ?cmd,
                    "router: session command routed"
                );
                Ok(RouterResult::Session(RouterSuccess { route }))
            } else {
                tracing::warn!(
                    guild_id = ?session.guild_id,
                    channel_id = ?session.channel_id,
                    cmd = ?cmd,
                    "router: session command for unknown session (NotJoined)"
                );
                Err(RouterError::NotJoined)
            }
        }
        AudioEngineCommand::FetchDiscordVoiceState => Err(RouterError::NotJoined),
    }
}

/// The AE id a worker's traffic routes to. Under the pod-ordinal registration scheme every
/// worker has exactly one live AE, always registered as `AeId(1)` (stale ones are evicted on
/// (re)register), so bot identity is fully determined by the worker. We route to that single
/// AE, defaulting to `1` even before the connected list is observed.
fn ae_id_for(worker: &Worker) -> u16 {
    worker.connected_ae_ids.first().copied().unwrap_or(1)
}

/// Rank the workers eligible to serve `session` in deterministic priority order using
/// rendezvous / highest-random-weight (HRW) hashing of `(worker_id, guild_id, channel_id)`.
///
/// The ranking is a pure function of the session key and the worker set — it does not depend
/// on any mutable cursor or on call order — so the same session always resolves to the same
/// worker across cache loss, retries, and TL restarts. That is what makes Join idempotent at
/// the bot-identity level and stops a second physical bot from ever joining one channel.
///
/// A worker is eligible when it (a) is permitted for the guild, (b) has a connected AE, and
/// (c) is not already serving a *different* channel in the same guild (Discord allows a token
/// only one voice connection per guild). Collisions on (c) resolve by walking down the fixed
/// HRW ranking, keeping multi-channel-per-guild placement stable too.
fn rank_workers_for_session<'a>(state: &'a ZakoState, session: &SessionInfo) -> Vec<&'a Worker> {
    let occupied = occupied_workers_for_other_channel(state, session);

    let mut eligible: Vec<&Worker> = state
        .workers
        .values()
        .filter(|worker| {
            let has_access =
                state.worker_has_access_to_guild(&worker.worker_id, &session.guild_id);
            let has_ae = !worker.connected_ae_ids.is_empty();
            let free = !occupied.contains(&worker.worker_id);
            tracing::debug!(
                worker_id = worker.worker_id.0,
                has_guild_access = has_access,
                connected_aes = worker.connected_ae_ids.len(),
                free_in_guild = free,
                eligible = has_access && has_ae && free,
                "router: worker eligibility check"
            );
            has_access && has_ae && free
        })
        .collect();

    // Descending HRW weight; ties broken by worker_id for a total, stable order.
    eligible.sort_by(|a, b| {
        let wa = hrw_weight(a.worker_id, session);
        let wb = hrw_weight(b.worker_id, session);
        wb.cmp(&wa).then(a.worker_id.0.cmp(&b.worker_id.0))
    });
    eligible
}

/// HRW weight for a worker/session pair. Uses FxHasher, which is seedless and therefore
/// deterministic across processes and restarts (unlike the randomly-seeded default HashMap
/// hasher), so the ranking is reproducible everywhere.
fn hrw_weight(worker_id: WorkerId, session: &SessionInfo) -> u64 {
    let mut h = FxHasher::default();
    worker_id.0.hash(&mut h);
    u64::from(session.guild_id).hash(&mut h);
    u64::from(session.channel_id).hash(&mut h);
    h.finish()
}

/// Workers already serving this guild for a *different* channel. These cannot take another
/// session in the same guild (one voice connection per token per guild). A worker serving
/// this exact session is not excluded — that is the idempotent existing route.
fn occupied_workers_for_other_channel(state: &ZakoState, session: &SessionInfo) -> Vec<WorkerId> {
    let guild_id: GuildId = session.guild_id;
    state
        .sessions_by_guild_id(guild_id)
        .iter()
        .filter(|(_, info)| info.channel_id != session.channel_id)
        .map(|(route, _)| route.worker_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiscordToken, WorkerPermissions};
    use rustc_hash::FxHashMap;
    use std::collections::HashMap;
    use zako3_types::{ChannelId, hq::DiscordUserId};

    fn worker(id: u16, guild: u64, connected: bool) -> Worker {
        let permissions = WorkerPermissions::new();
        permissions.add_allowed_guild(GuildId::from(guild));
        Worker {
            worker_id: WorkerId(id),
            bot_client_id: DiscordUserId(String::new()),
            discord_token: DiscordToken(String::new()),
            connected_ae_ids: if connected { vec![1] } else { vec![] },
            permissions,
        }
    }

    /// State with `n` workers, all permitted for `guild` and each with one connected AE.
    fn state_with_workers(n: u16, guild: u64) -> ZakoState {
        let mut workers = FxHashMap::default();
        for id in 0..n {
            workers.insert(WorkerId(id), worker(id, guild, true));
        }
        ZakoState {
            workers,
            sessions: Default::default(),
        }
    }

    fn join_req(guild: u64, channel: u64) -> AudioEngineCommandRequest {
        AudioEngineCommandRequest {
            session: Some(SessionInfo {
                guild_id: GuildId::from(guild),
                channel_id: ChannelId::from(channel),
            }),
            command: AudioEngineCommand::Join,
            headers: HashMap::new(),
            idempotency_key: None,
        }
    }

    fn first_route(state: &ZakoState, guild: u64, channel: u64) -> SessionRoute {
        match route(state, &join_req(guild, channel)).unwrap() {
            RouterResult::Join(c) => c[0].route,
            _ => panic!("expected Join"),
        }
    }

    #[test]
    fn join_placement_is_deterministic_across_calls() {
        let state = state_with_workers(5, 1);
        let a = first_route(&state, 1, 100);
        let b = first_route(&state, 1, 100);
        assert_eq!(a, b, "same session key must always route to the same worker");
    }

    #[test]
    fn join_placement_independent_of_worker_insertion_order() {
        // Build the same worker set in two different insertion orders; HRW ranking must agree.
        let guild = 7u64;
        let mut fwd = FxHashMap::default();
        for id in 0..6 {
            fwd.insert(WorkerId(id), worker(id, guild, true));
        }
        let mut rev = FxHashMap::default();
        for id in (0..6).rev() {
            rev.insert(WorkerId(id), worker(id, guild, true));
        }
        let s_fwd = ZakoState { workers: fwd, sessions: Default::default() };
        let s_rev = ZakoState { workers: rev, sessions: Default::default() };
        assert_eq!(
            first_route(&s_fwd, guild, 42),
            first_route(&s_rev, guild, 42),
            "placement must not depend on HashMap iteration order"
        );
    }

    #[test]
    fn join_same_route_after_cache_loss() {
        // A "lost commit": the first Join picked worker W but nothing was cached. Re-routing
        // with an empty cache must resolve to the exact same route — that is what stops a
        // different bot from joining on retry.
        let state = state_with_workers(4, 1);
        let original = first_route(&state, 1, 100);
        // cache still empty (commit lost)
        let retry = first_route(&state, 1, 100);
        assert_eq!(original, retry);
    }

    #[test]
    fn join_reuses_existing_cached_route_first() {
        // Whatever route is already cached for the session must be offered first (idempotent
        // re-Join → AE answers AlreadyJoined), even if it isn't the HRW top.
        let mut state = state_with_workers(4, 1);
        let session = SessionInfo {
            guild_id: GuildId::from(1u64),
            channel_id: ChannelId::from(100u64),
        };
        // Pick a route that is NOT the deterministic top to prove "existing wins".
        let top = first_route(&state, 1, 100);
        let other = (0..4)
            .map(|id| SessionRoute { worker_id: WorkerId(id), ae_id: AeId(1) })
            .find(|r| *r != top)
            .unwrap();
        state.sessions.insert(other, session);
        assert_eq!(first_route(&state, 1, 100), other);
    }

    #[test]
    fn two_channels_same_guild_get_distinct_workers() {
        // Discord allows one voice connection per token per guild: two channels in one guild
        // must land on two different workers, deterministically.
        let mut state = state_with_workers(4, 1);
        let r1 = first_route(&state, 1, 100);
        state.sessions.insert(
            r1,
            SessionInfo { guild_id: GuildId::from(1u64), channel_id: ChannelId::from(100u64) },
        );
        let r2 = first_route(&state, 1, 200);
        assert_ne!(r1.worker_id, r2.worker_id, "second channel must use a different worker");
        // Stable: recomputing the second placement yields the same worker.
        assert_eq!(r2, first_route(&state, 1, 200));
    }

    #[test]
    fn no_eligible_worker_is_rejected() {
        // Worker exists but has no connected AE and no guild permission → NoAvailableWorker.
        let mut workers = FxHashMap::default();
        workers.insert(WorkerId(0), worker(0, 999, false)); // wrong guild, no AE
        let state = ZakoState { workers, sessions: Default::default() };
        assert!(matches!(
            route(&state, &join_req(1, 100)),
            Err(RouterError::NoAvailableWorker)
        ));
    }

    #[test]
    fn candidates_cover_all_eligible_workers_deterministically() {
        // The candidate list must span every eligible worker (failover), in a fixed order.
        let state = state_with_workers(3, 1);
        let list = match route(&state, &join_req(1, 100)).unwrap() {
            RouterResult::Join(c) => c,
            _ => panic!("expected Join"),
        };
        assert_eq!(list.len(), 3, "all eligible workers are failover candidates");
        let again = match route(&state, &join_req(1, 100)).unwrap() {
            RouterResult::Join(c) => c,
            _ => panic!("expected Join"),
        };
        let ids: Vec<_> = list.iter().map(|c| c.route.worker_id).collect();
        let ids2: Vec<_> = again.iter().map(|c| c.route.worker_id).collect();
        assert_eq!(ids, ids2, "candidate order is stable across calls");
    }
}
