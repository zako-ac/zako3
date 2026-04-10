use async_trait::async_trait;
use dashmap::DashMap;
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::state::StateService,
    types::{ChannelId, GuildId, SessionState},
};

pub struct InMemoryStateService {
    sessions: DashMap<(GuildId, ChannelId), SessionState>,
}

impl InMemoryStateService {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }
}

impl Default for InMemoryStateService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateService for InMemoryStateService {
    async fn get_session(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> ZakoResult<Option<SessionState>> {
        Ok(self
            .sessions
            .get(&(guild_id, channel_id))
            .map(|s| s.value().clone()))
    }

    async fn save_session(&self, session: &SessionState) -> ZakoResult<()> {
        self.sessions
            .insert((session.guild_id, session.channel_id), session.clone());
        Ok(())
    }

    async fn delete_session(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        self.sessions.remove(&(guild_id, channel_id));
        Ok(())
    }

    async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>> {
        Ok(self.sessions.iter().map(|s| s.value().clone()).collect())
    }

    async fn list_sessions_in_guild(&self, guild_id: GuildId) -> ZakoResult<Vec<SessionState>> {
        Ok(self
            .sessions
            .iter()
            .filter(|s| s.key().0 == guild_id)
            .map(|s| s.value().clone())
            .collect())
    }
}
