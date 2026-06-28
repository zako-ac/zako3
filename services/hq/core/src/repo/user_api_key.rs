use crate::CoreResult;
use async_trait::async_trait;
use chrono::Utc;
use hq_types::hq::{UserApiKey, UserApiKeyId, UserId};
use sqlx::{PgPool, Row};

#[async_trait]
pub trait UserApiKeyRepository: Send + Sync {
    async fn create(&self, key: &UserApiKey) -> CoreResult<UserApiKey>;
    async fn list_by_user(&self, user_id: UserId) -> CoreResult<Vec<UserApiKey>>;
    async fn find_by_id(&self, id: UserApiKeyId) -> CoreResult<Option<UserApiKey>>;
    /// Active = not revoked. Used by the auth path to validate a presented jti.
    async fn find_active(&self, id: UserApiKeyId) -> CoreResult<Option<UserApiKey>>;
    async fn update_label(&self, id: UserApiKeyId, label: &str) -> CoreResult<UserApiKey>;
    async fn revoke(&self, id: UserApiKeyId) -> CoreResult<()>;
    async fn touch_last_used(&self, id: UserApiKeyId) -> CoreResult<()>;
}

pub struct PgUserApiKeyRepository {
    pool: PgPool,
}

impl PgUserApiKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn row_to_key(row: &sqlx::postgres::PgRow) -> CoreResult<UserApiKey> {
    Ok(UserApiKey {
        id: UserApiKeyId(row.try_get("id")?),
        user_id: UserId(row.try_get("user_id")?),
        label: row.try_get("label")?,
        expires_at: row.try_get("expires_at")?,
        last_used_at: row.try_get("last_used_at")?,
        revoked_at: row.try_get("revoked_at")?,
        created_at: row.try_get("created_at")?,
    })
}

#[async_trait]
impl UserApiKeyRepository for PgUserApiKeyRepository {
    async fn create(&self, key: &UserApiKey) -> CoreResult<UserApiKey> {
        let row = sqlx::query(
            r#"
            INSERT INTO user_api_keys (id, user_id, label, expires_at, created_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, user_id, label, expires_at, last_used_at, revoked_at, created_at
            "#,
        )
        .bind(key.id.0.clone())
        .bind(key.user_id.0.clone())
        .bind(key.label.clone())
        .bind(key.expires_at)
        .bind(key.created_at)
        .fetch_one(&self.pool)
        .await?;

        row_to_key(&row)
    }

    async fn list_by_user(&self, user_id: UserId) -> CoreResult<Vec<UserApiKey>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, label, expires_at, last_used_at, revoked_at, created_at
            FROM user_api_keys
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_key).collect()
    }

    async fn find_by_id(&self, id: UserApiKeyId) -> CoreResult<Option<UserApiKey>> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, user_id, label, expires_at, last_used_at, revoked_at, created_at
            FROM user_api_keys
            WHERE id = $1
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;

        row_opt.as_ref().map(row_to_key).transpose()
    }

    async fn find_active(&self, id: UserApiKeyId) -> CoreResult<Option<UserApiKey>> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, user_id, label, expires_at, last_used_at, revoked_at, created_at
            FROM user_api_keys
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;

        row_opt.as_ref().map(row_to_key).transpose()
    }

    async fn update_label(&self, id: UserApiKeyId, label: &str) -> CoreResult<UserApiKey> {
        let row = sqlx::query(
            r#"
            UPDATE user_api_keys
            SET label = $1
            WHERE id = $2
            RETURNING id, user_id, label, expires_at, last_used_at, revoked_at, created_at
            "#,
        )
        .bind(label)
        .bind(id.0)
        .fetch_one(&self.pool)
        .await?;

        row_to_key(&row)
    }

    async fn revoke(&self, id: UserApiKeyId) -> CoreResult<()> {
        sqlx::query(
            "UPDATE user_api_keys SET revoked_at = $1 WHERE id = $2 AND revoked_at IS NULL",
        )
        .bind(Utc::now())
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn touch_last_used(&self, id: UserApiKeyId) -> CoreResult<()> {
        sqlx::query("UPDATE user_api_keys SET last_used_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id.0)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
