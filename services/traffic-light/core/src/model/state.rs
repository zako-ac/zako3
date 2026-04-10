use crate::*;
use rustc_hash::FxHashMap;
use tl_protocol::SessionInfo;
use zako3_types::{GuildId, hq::DiscordUserId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct SessionRoute {
    pub worker_id: WorkerId,
    pub ae_id: AeId,
}

#[derive(Clone, Debug)]
pub struct Worker {
    pub worker_id: WorkerId,
    pub bot_client_id: DiscordUserId,
    pub discord_token: DiscordToken,
    pub connected_ae_ids: Vec<u16>,
    pub permissions: WorkerPermissions,
    pub ae_cursor: u16,
}

#[derive(Clone, Debug)]
pub struct ZakoState {
    pub workers: FxHashMap<WorkerId, Worker>,
    pub sessions: FxHashMap<SessionRoute, SessionInfo>,
    pub worker_cursor: u16,
}

impl ZakoState {
    pub fn session_by_info(&self, session_info: &SessionInfo) -> Option<SessionRoute> {
        self.sessions
            .iter()
            .find(|(_, info)| *info == session_info)
            .map(|(route, _)| *route)
    }

    pub fn sessions_by_guild_id(&self, guild_id: GuildId) -> Vec<(SessionRoute, SessionInfo)> {
        self.sessions
            .iter()
            .filter(|(_, info)| info.guild_id == guild_id)
            .map(|(route, info)| (*route, *info))
            .collect()
    }

    pub fn sessions_by_worker(&self, worker_id: WorkerId) -> Vec<(SessionRoute, SessionInfo)> {
        self.sessions
            .iter()
            .filter(|(route, _)| route.worker_id == worker_id)
            .map(|(route, info)| (*route, *info))
            .collect()
    }

    pub fn worker_by_bot_client_id(&self, bot_client_id: &DiscordUserId) -> Option<WorkerId> {
        self.workers
            .iter()
            .find(|(_, worker)| &worker.bot_client_id == bot_client_id)
            .map(|(worker_id, _)| *worker_id)
    }

    #[inline]
    pub fn worker_has_access_to_guild(&self, worker_id: &WorkerId, guild_id: &GuildId) -> bool {
        self.workers
            .get(worker_id)
            .map(|worker| worker.permissions.is_guild_allowed(guild_id))
            .unwrap_or(false)
    }
}
