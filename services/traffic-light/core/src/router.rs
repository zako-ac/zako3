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
    match request.command {
        AudioEngineCommand::Join => {
            if let Some(existing_route) = state.session_by_info(&request.session) {
                // Session already tracked — try the existing AE first (state-drift recovery),
                // then retry the same route as a fresh join in case the AE lost its session
                // after a restart. Using the same route avoids a second bot joining the channel.
                let mut fresh_state = state.clone();
                fresh_state.sessions.insert(existing_route, request.session);
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

            let eligible_workers: Vec<&Worker> =
                available_workers_for_guild(state, request.session.guild_id)
                    .into_iter()
                    .filter(|worker| {
                        tracing::debug!(
                            worker_id = worker.worker_id.0,
                            "Checking worker eligibility for Join command: {:#?}",
                            state.workers
                        );
                        state.worker_has_access_to_guild(
                            &worker.worker_id,
                            &request.session.guild_id,
                        )
                    })
                    .filter(|worker| !worker.connected_ae_ids.is_empty())
                    .collect();

            if eligible_workers.is_empty() {
                return Err(RouterError::NoAvailableWorker);
            }

            // Build all candidates in round-robin order starting from worker_cursor.
            let n = eligible_workers.len();
            let start = (state.worker_cursor as usize + 1) % n;
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
                    new_state.sessions.insert(route, request.session);
                    RouterSuccess {
                        route,
                        new_state_on_success: new_state,
                    }
                })
                .collect();

            Ok(RouterResult::Join(candidates))
        }
        AudioEngineCommand::SessionCommand(_) => {
            if let Some(route) = state.session_by_info(&request.session) {
                Ok(RouterResult::Session(RouterSuccess {
                    route,
                    new_state_on_success: state.clone(),
                }))
            } else {
                Err(RouterError::NotJoined)
            }
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
