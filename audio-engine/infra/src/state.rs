use async_trait::async_trait;
use redis::AsyncCommands;
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::state::StateService,
    types::{GuildId, SessionState},
};

pub struct RedisStateService {
    conn: redis::aio::MultiplexedConnection,
}

impl RedisStateService {
    pub async fn new(redis_url: &str) -> ZakoResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_async_connection().await?;
        Ok(Self { conn })
    }

    fn get_key(guild_id: GuildId) -> String {
        let id: u64 = guild_id.into();
        format!("session:{}", id)
    }
}

#[async_trait]
impl StateService for RedisStateService {
    async fn get_session(&self, guild_id: GuildId) -> ZakoResult<Option<SessionState>> {
        let mut conn = self.conn.clone();
        let key = Self::get_key(guild_id);
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
        let key = Self::get_key(session.guild_id);
        let json = serde_json::to_string(session)?;
        let _: () = conn.set(key, json).await?;
        Ok(())
    }

    async fn delete_session(&self, guild_id: GuildId) -> ZakoResult<()> {
        let mut conn = self.conn.clone();
        let key = Self::get_key(guild_id);
        let _: () = conn.del(key).await?;
        Ok(())
    }
}
