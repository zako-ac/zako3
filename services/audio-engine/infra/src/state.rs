use async_trait::async_trait;
use redis::AsyncCommands;
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::state::StateService,
    types::{ChannelId, GuildId, SessionState},
};

pub struct RedisStateService {
    conn: redis::aio::MultiplexedConnection,
    ae_id: String,
}

impl RedisStateService {
    pub async fn new(redis_url: &str, ae_id: String) -> ZakoResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_async_connection().await?;
        Ok(Self { conn, ae_id })
    }

    fn get_key(&self, guild_id: GuildId, channel_id: ChannelId) -> String {
        format!(
            "session:{}:{}:{}",
            self.ae_id,
            u64::from(guild_id),
            u64::from(channel_id)
        )
    }

    fn guild_pattern(&self, guild_id: GuildId) -> String {
        format!("session:{}:{}:*", self.ae_id, u64::from(guild_id))
    }
}

#[async_trait]
impl StateService for RedisStateService {
    async fn get_session(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> ZakoResult<Option<SessionState>> {
        let mut conn = self.conn.clone();
        let key = self.get_key(guild_id, channel_id);
        let data: Option<String> = conn.get(key).await?;

        match data {
            Some(json) => {
                let session: SessionState = serde_json::from_str(&json)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn save_session(&self, session: &SessionState) -> ZakoResult<()> {
        let mut conn = self.conn.clone();
        let key = self.get_key(session.guild_id, session.channel_id);
        let json = serde_json::to_string(session)?;
        let _: () = conn.set(key, json).await?;
        Ok(())
    }

    async fn delete_session(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        let mut conn = self.conn.clone();
        let key = self.get_key(guild_id, channel_id);
        let _: () = conn.del(key).await?;
        Ok(())
    }

    async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>> {
        let mut conn = self.conn.clone();
        let pattern = format!("session:{}:*", self.ae_id);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await?;

        let mut sessions = Vec::with_capacity(keys.len());
        for key in keys {
            let data: Option<String> = conn.get(&key).await?;
            if let Some(json) = data {
                let session: SessionState = serde_json::from_str(&json)?;
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    async fn list_sessions_in_guild(&self, guild_id: GuildId) -> ZakoResult<Vec<SessionState>> {
        let mut conn = self.conn.clone();
        let pattern = self.guild_pattern(guild_id);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await?;

        let mut sessions = Vec::with_capacity(keys.len());
        for key in keys {
            let data: Option<String> = conn.get(&key).await?;
            if let Some(json) = data {
                let session: SessionState = serde_json::from_str(&json)?;
                sessions.push(session);
            }
        }

        Ok(sessions)
    }
}
