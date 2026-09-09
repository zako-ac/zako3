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
}

#[derive(Clone, Debug)]
pub struct ZakoState {
    pub workers: FxHashMap<WorkerId, Worker>,
    /// One bot (`SessionRoute`) can legitimately hold a voice connection in *many guilds at
    /// once*. Discord enforces at most one voice connection per token per guild, so each route
    /// holds at most one session per `GuildId` — the inner map keyed by guild makes that
    /// invariant explicit. (Previously a flat `HashMap<SessionRoute, SessionInfo>` could track
    /// only one session per bot, silently dropping every other guild's session and making the
    /// bot "invisible" to HQ in those guilds.)
    pub sessions: FxHashMap<SessionRoute, FxHashMap<GuildId, SessionInfo>>,
}

impl ZakoState {
    /// The route tracking the given exact session (used for idempotent re-Join). Sessions are
    /// unique per `(route, guild)`, so at most one route can match a given `SessionInfo`.
    pub fn session_by_info(&self, session_info: &SessionInfo) -> Option<SessionRoute> {
        self.sessions
            .iter()
            .find(|(_, by_guild)| by_guild.values().any(|info| info == session_info))
            .map(|(route, _)| *route)
    }

    pub fn sessions_by_guild_id(&self, guild_id: GuildId) -> Vec<(SessionRoute, SessionInfo)> {
        self.sessions
            .iter()
            .filter_map(|(route, by_guild)| {
                by_guild.get(&guild_id).map(|info| (*route, *info))
            })
            .collect()
    }

    pub fn sessions_by_worker(&self, worker_id: WorkerId) -> Vec<(SessionRoute, SessionInfo)> {
        self.sessions
            .iter()
            .filter(|(route, _)| route.worker_id == worker_id)
            .flat_map(|(route, by_guild)| {
                by_guild.values().map(|info| (*route, *info))
            })
            .collect()
    }

    /// All live sessions tracked for a single route (a bot in multiple guilds at once).
    pub fn sessions_for_route(&self, route: &SessionRoute) -> Vec<SessionInfo> {
        self.sessions
            .get(route)
            .map(|by_guild| by_guild.values().copied().collect())
            .unwrap_or_default()
    }

    /// Every `(route, session)` pair currently tracked, flattened across guilds.
    pub fn all_sessions(&self) -> Vec<(SessionRoute, SessionInfo)> {
        self.sessions
            .iter()
            .flat_map(|(route, by_guild)| {
                by_guild.values().map(|info| (*route, *info))
            })
            .collect()
    }

    /// Insert one session for `route`. The inner map is keyed by the session's guild, so
    /// inserting a session for an already-tracked guild simply updates its channel.
    pub fn insert_session(&mut self, route: SessionRoute, info: SessionInfo) {
        self.sessions
            .entry(route)
            .or_default()
            .insert(info.guild_id, info);
    }

    /// Remove the session for `route` in `guild_id`. Removes the whole route entry when the
    /// bot no longer has any session. Returns whether a session was removed.
    pub fn remove_session(&mut self, route: &SessionRoute, guild_id: GuildId) -> bool {
        let removed = self
            .sessions
            .get_mut(route)
            .map(|by_guild| by_guild.remove(&guild_id).is_some())
            .unwrap_or(false);
        if removed {
            let empty = self
                .sessions
                .get(route)
                .map(|by_guild| by_guild.is_empty())
                .unwrap_or(false);
            if empty {
                self.sessions.remove(route);
            }
        }
        removed
    }

    /// Total number of tracked sessions across every route (a bot in two guilds counts as 2).
    pub fn total_session_count(&self) -> usize {
        self.sessions.values().map(|by_guild| by_guild.len()).sum()
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
