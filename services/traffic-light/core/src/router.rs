use thiserror::Error;
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
    pub new_state_on_success: ZakoState,
}

/// Result of a routing decision.
pub enum RouterResult {
    /// All viable Join candidates in round-robin order. TlService tries them sequentially.
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
            if let Some(existing_route) = state.session_by_info(&session) {
                // Session already tracked — try the existing AE first (state-drift recovery),
                // then retry the same route as a fresh join in case the AE lost its session
                // after a restart. Using the same route avoids a second bot joining the channel.
                tracing::info!(
                    worker_id = existing_route.worker_id.0,
                    ae_id = existing_route.ae_id.0,
                    guild_id = ?session.guild_id,
                    channel_id = ?session.channel_id,
                    "router: Join for already-tracked session, reusing existing route (state-drift recovery)"
                );
                let mut fresh_state = state.clone();
                fresh_state.sessions.insert(existing_route, session);
                return Ok(RouterResult::Join(vec![
                    RouterSuccess {
                        route: existing_route,
                        new_state_on_success: state.clone(),
                    },
                    RouterSuccess {
                        route: existing_route,
                        new_state_on_success: fresh_state,
                    },
                ]));
            }

            let available = available_workers_for_guild(state, session.guild_id);
            tracing::debug!(
                guild_id = ?session.guild_id,
                available_count = available.len(),
                total_workers = state.workers.len(),
                "router: available workers for guild (not already serving this guild)"
            );

            let eligible_workers: Vec<&Worker> = available
                    .into_iter()
                    .filter(|worker| {
                        let has_access = state.worker_has_access_to_guild(
                            &worker.worker_id,
                            &session.guild_id,
                        );
                        let has_ae = !worker.connected_ae_ids.is_empty();
                        tracing::debug!(
                            worker_id = worker.worker_id.0,
                            has_guild_access = has_access,
                            connected_aes = worker.connected_ae_ids.len(),
                            eligible = has_access && has_ae,
                            "router: worker eligibility check"
                        );
                        has_access && has_ae
                    })
                    .collect();

            if eligible_workers.is_empty() {
                tracing::warn!(
                    guild_id = ?session.guild_id,
                    total_workers = state.workers.len(),
                    "router: no eligible worker for Join (no access or no connected AEs)"
                );
                return Err(RouterError::NoAvailableWorker);
            }

            // Build all candidates in round-robin order starting from worker_cursor.
            let n = eligible_workers.len();
            let start = (state.worker_cursor as usize + 1) % n;
            tracing::info!(
                guild_id = ?session.guild_id,
                channel_id = ?session.channel_id,
                eligible_workers = n,
                first_candidate_worker_id = eligible_workers[start % n].worker_id.0,
                "router: building Join candidates"
            );
            let candidates = (0..n)
                .map(|i| {
                    let worker = eligible_workers[(start + i) % n];
                    let mut new_state = state.clone();
                    new_state.worker_cursor = ((start + i) % n) as u16;

                    let ae_start = (worker.ae_cursor as usize + 1) % worker.connected_ae_ids.len();
                    let ae_id = AeId(worker.connected_ae_ids[ae_start]);
                    let worker_mut = new_state.workers.get_mut(&worker.worker_id).unwrap();
                    worker_mut.ae_cursor = ae_start as u16;

                    let route = SessionRoute {
                        worker_id: worker.worker_id,
                        ae_id,
                    };
                    new_state.sessions.insert(route, session);
                    RouterSuccess {
                        route,
                        new_state_on_success: new_state,
                    }
                })
                .collect();

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
                Ok(RouterResult::Session(RouterSuccess {
                    route,
                    new_state_on_success: state.clone(),
                }))
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
        AudioEngineCommand::FetchDiscordVoiceState => {
            Err(RouterError::NotJoined)
        }
    }
}

fn available_workers_for_guild<'a>(state: &'a ZakoState, guild_id: GuildId) -> Vec<&'a Worker> {
    let occupied_workers = occupied_workers_for_guild(state, guild_id);
    state
        .workers
        .iter()
        .filter(|(worker_id, _)| !occupied_workers.contains(worker_id))
        .map(|(_, worker)| worker)
        .collect()
}

fn occupied_workers_for_guild(state: &ZakoState, guild_id: GuildId) -> Vec<WorkerId> {
    state
        .sessions_by_guild_id(guild_id)
        .iter()
        .map(|(route, _)| route.worker_id)
        .collect()
}
