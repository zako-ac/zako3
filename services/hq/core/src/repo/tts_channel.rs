use crate::CoreResult;
use async_trait::async_trait;
use hq_types::{ChannelId, GuildId};

#[async_trait]
pub trait TtsChannelRepo: Send + Sync {
    async fn set_enabled(
        &self,
        guild_id: &GuildId,
        channel_id: &ChannelId,
        enabled: bool,
    ) -> CoreResult<()>;
    async fn is_enabled(&self, channel_id: &ChannelId) -> CoreResult<bool>;
    async fn get_enabled_channels(&self, guild_id: &GuildId) -> CoreResult<Vec<ChannelId>>;
}

pub struct PgTtsChannelRepo {
    pool: sqlx::PgPool,
}

impl PgTtsChannelRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TtsChannelRepo for PgTtsChannelRepo {
    async fn set_enabled(
        &self,
        guild_id: &GuildId,
        channel_id: &ChannelId,
        enabled: bool,
    ) -> CoreResult<()> {
        let g_id: i64 = u64::from(*guild_id) as i64;
        let c_id: i64 = u64::from(*channel_id) as i64;

        if enabled {
            sqlx::query("INSERT INTO enabled_tts_channels (guild_id, channel_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(g_id)
                .bind(c_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM enabled_tts_channels WHERE guild_id = $1 AND channel_id = $2")
                .bind(g_id)
                .bind(c_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn is_enabled(&self, channel_id: &ChannelId) -> CoreResult<bool> {
        let c_id: i64 = u64::from(*channel_id) as i64;
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM enabled_tts_channels WHERE channel_id = $1")
                .bind(c_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    async fn get_enabled_channels(&self, guild_id: &GuildId) -> CoreResult<Vec<ChannelId>> {
        let g_id: i64 = u64::from(*guild_id) as i64;
        let rows: Vec<(i64,)> =
            sqlx::query_as("SELECT channel_id FROM enabled_tts_channels WHERE guild_id = $1")
                .bind(g_id)
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .map(|(cid,)| ChannelId::from(cid as u64))
            .collect())
    }
}
