use async_trait::async_trait;
use hq_types::hq::{settings::PartialUserSettings, UserId};
use sqlx::{PgPool, Row};

use crate::CoreResult;

#[async_trait]
pub trait UserGuildSettingsRepository: Send + Sync {
    async fn get(&self, user_id: &UserId, guild_id: &str) -> CoreResult<Option<PartialUserSettings>>;
    async fn upsert(&self, user_id: &UserId, guild_id: &str, settings: &PartialUserSettings) -> CoreResult<PartialUserSettings>;
    async fn delete(&self, user_id: &UserId, guild_id: &str) -> CoreResult<()>;
}

pub struct PgUserGuildSettingsRepository {
    pool: PgPool,
}

impl PgUserGuildSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserGuildSettingsRepository for PgUserGuildSettingsRepository {
    async fn get(&self, user_id: &UserId, guild_id: &str) -> CoreResult<Option<PartialUserSettings>> {
        let row = sqlx::query(
            "SELECT settings FROM user_guild_settings WHERE user_id = $1 AND guild_id = $2",
        )
        .bind(&user_id.0)
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

    async fn upsert(&self, user_id: &UserId, guild_id: &str, settings: &PartialUserSettings) -> CoreResult<PartialUserSettings> {
        let json = serde_json::to_value(settings)?;

        let row = sqlx::query(
            r#"
            INSERT INTO user_guild_settings (user_id, guild_id, settings)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, guild_id) DO UPDATE
                SET settings = EXCLUDED.settings, updated_at = now()
            RETURNING settings
            "#,
        )
        .bind(&user_id.0)
        .bind(guild_id)
        .bind(json)
        .fetch_one(&self.pool)
        .await?;

        let value: serde_json::Value = row.try_get("settings")?;
        Ok(serde_json::from_value(value)?)
    }

    async fn delete(&self, user_id: &UserId, guild_id: &str) -> CoreResult<()> {
        sqlx::query(
            "DELETE FROM user_guild_settings WHERE user_id = $1 AND guild_id = $2",
        )
        .bind(&user_id.0)
        .bind(guild_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
