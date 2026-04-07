use async_trait::async_trait;
use hq_types::hq::settings::PartialUserSettings;
use sqlx::{PgPool, Row};

use crate::CoreResult;

#[async_trait]
pub trait GlobalSettingsRepository: Send + Sync {
    async fn get(&self) -> CoreResult<Option<PartialUserSettings>>;
    async fn upsert(&self, settings: &PartialUserSettings) -> CoreResult<PartialUserSettings>;
}

pub struct PgGlobalSettingsRepository {
    pool: PgPool,
}

impl PgGlobalSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GlobalSettingsRepository for PgGlobalSettingsRepository {
    async fn get(&self) -> CoreResult<Option<PartialUserSettings>> {
        let row = sqlx::query("SELECT settings FROM global_settings WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = row else { return Ok(None) };
        let value: Option<serde_json::Value> = row.try_get("settings")?;
        let Some(value) = value else { return Ok(None) };
        Ok(Some(serde_json::from_value(value)?))
    }

    async fn upsert(&self, settings: &PartialUserSettings) -> CoreResult<PartialUserSettings> {
        let json = serde_json::to_value(settings)?;

        let row = sqlx::query(
            r#"
            INSERT INTO global_settings (id, settings)
            VALUES (1, $1)
            ON CONFLICT (id) DO UPDATE
                SET settings = EXCLUDED.settings, updated_at = now()
            RETURNING settings
            "#,
        )
        .bind(json)
        .fetch_one(&self.pool)
        .await?;

        let value: serde_json::Value = row.try_get("settings")?;
        Ok(serde_json::from_value(value)?)
    }
}
