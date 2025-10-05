use async_trait::async_trait;

use sqlx::{Row, types::Json};

use crate::{
    feature::settings::{
        Settings, SettingsObject, repository::SettingsRepository, scope::SettingsScope,
    },
    infrastructure::postgres::PostgresDb,
    util::error::AppResult,
};

#[async_trait]
impl SettingsRepository for PostgresDb {
    async fn get_settings(&self, scope: &SettingsScope) -> AppResult<Option<SettingsObject>> {
        let query = sqlx::query("SELECT * FROM settings WHERE scope = $1").bind(scope.as_key());

        let s = query.fetch_optional(&self.pool).await?;
        if let Some(s) = s {
            let data: Json<SettingsObject> = s.try_get("data")?;
            Ok(Some(data.0))
        } else {
            Ok(None)
        }
    }

    async fn set_settings(&self, settings: &Settings) -> AppResult<()> {
        let query = sqlx::query(
            "INSERT INTO settings (scope, data) VALUES ($1, $2) ON CONFLICT (scope) DO UPDATE SET data = EXCLUDED.data;",
        ).bind(settings.scope.as_key()).bind(Json(settings.object.clone()));

        query.execute(&self.pool).await?;

        Ok(())
    }

    async fn delete_settings(&self, scope: &SettingsScope) -> AppResult<()> {
        let query = sqlx::query("DELETE FROM settings WHERE scope = $1").bind(scope.as_key());

        query.execute(&self.pool).await?;

        Ok(())
    }
}
