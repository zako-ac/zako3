use async_trait::async_trait;
use hq_types::hq::settings::PartialUserSettings;
use sqlx::{PgPool, Row};

use crate::CoreResult;

#[async_trait]
pub trait GuildSettingsRepository: Send + Sync {
    async fn get(&self, guild_id: &str) -> CoreResult<Option<PartialUserSettings>>;
    async fn upsert(
        &self,
        guild_id: &str,
        settings: &PartialUserSettings,
    ) -> CoreResult<PartialUserSettings>;
    async fn delete(&self, guild_id: &str) -> CoreResult<()>;
}

pub struct PgGuildSettingsRepository {
    pool: PgPool,
}

impl PgGuildSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GuildSettingsRepository for PgGuildSettingsRepository {
    async fn get(&self, guild_id: &str) -> CoreResult<Option<PartialUserSettings>> {
        let row = sqlx::query("SELECT settings FROM guild_settings WHERE guild_id = $1")
            .bind(guild_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = row else { return Ok(None) };
        let value: Option<serde_json::Value> = row.try_get("settings")?;
        let Some(value) = value else { return Ok(None) };
        // Treat deserialization errors as "no settings" (empty object in DB, malformed data, etc.)
        match serde_json::from_value(value) {
            Ok(settings) => Ok(Some(settings)),
            Err(_) => Ok(None),
        }
    }

    async fn upsert(
        &self,
        guild_id: &str,
        settings: &PartialUserSettings,
    ) -> CoreResult<PartialUserSettings> {
        let json = serde_json::to_value(settings)?;

        let row = sqlx::query(
            r#"
            INSERT INTO guild_settings (guild_id, settings)
            VALUES ($1, $2)
            ON CONFLICT (guild_id) DO UPDATE
                SET settings = EXCLUDED.settings, updated_at = now()
            RETURNING settings
            "#,
        )
        .bind(guild_id)
        .bind(json)
        .fetch_one(&self.pool)
        .await?;

        let value: serde_json::Value = row.try_get("settings")?;
        Ok(serde_json::from_value(value)?)
    }

    async fn delete(&self, guild_id: &str) -> CoreResult<()> {
        sqlx::query("DELETE FROM guild_settings WHERE guild_id = $1")
            .bind(guild_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
