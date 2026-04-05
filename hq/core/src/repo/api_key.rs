use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{ApiKey, ApiKeyId, TapId};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn create(&self, key: &ApiKey) -> CoreResult<ApiKey>;
    async fn list_by_tap(&self, tap_id: Uuid) -> CoreResult<Vec<ApiKey>>;
    async fn find_by_id(&self, id: Uuid) -> CoreResult<Option<ApiKey>>;
    async fn find_by_key_hash(&self, hash: &str) -> CoreResult<Option<ApiKey>>;
    async fn update(&self, key: &ApiKey) -> CoreResult<ApiKey>;
    async fn delete(&self, id: Uuid) -> CoreResult<()>;
}

pub struct PgApiKeyRepository {
    pool: PgPool,
}

impl PgApiKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ApiKeyRepository for PgApiKeyRepository {
    async fn create(&self, key: &ApiKey) -> CoreResult<ApiKey> {
        let id = key.id.0;
        let tap_id = key.tap_id.0;
        let name = key.name.clone();
        let key_hash = key.key_hash.clone();
        let scopes = serde_json::to_value(&key.scopes)?;
        let created_at = key.created_at;

        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (id, tap_id, name, key_hash, scopes, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, tap_id, name, key_hash, scopes, last_used_at, created_at
            "#,
        )
        .bind(id)
        .bind(tap_id)
        .bind(name)
        .bind(key_hash)
        .bind(scopes)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(ApiKey {
            id: ApiKeyId(row.try_get("id")?),
            tap_id: TapId(row.try_get("tap_id")?),
            name: row.try_get("name")?,
            key_hash: row.try_get("key_hash")?,
            scopes: serde_json::from_value(row.try_get("scopes")?)?,
            last_used_at: row.try_get("last_used_at")?,
            created_at: row.try_get("created_at")?,
        })
    }

    async fn list_by_tap(&self, tap_id: Uuid) -> CoreResult<Vec<ApiKey>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tap_id, name, key_hash, scopes, last_used_at, created_at
            FROM api_keys
            WHERE tap_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(tap_id)
        .fetch_all(&self.pool)
        .await?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(ApiKey {
                id: ApiKeyId(row.try_get("id")?),
                tap_id: TapId(row.try_get("tap_id")?),
                name: row.try_get("name")?,
                key_hash: row.try_get("key_hash")?,
                scopes: serde_json::from_value(row.try_get("scopes")?)?,
                last_used_at: row.try_get("last_used_at")?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(keys)
    }

    async fn find_by_id(&self, id: Uuid) -> CoreResult<Option<ApiKey>> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, tap_id, name, key_hash, scopes, last_used_at, created_at
            FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row_opt {
            Ok(Some(ApiKey {
                id: ApiKeyId(row.try_get("id")?),
                tap_id: TapId(row.try_get("tap_id")?),
                name: row.try_get("name")?,
                key_hash: row.try_get("key_hash")?,
                scopes: serde_json::from_value(row.try_get("scopes")?)?,
                last_used_at: row.try_get("last_used_at")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_by_key_hash(&self, hash: &str) -> CoreResult<Option<ApiKey>> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, tap_id, name, key_hash, scopes, last_used_at, created_at
            FROM api_keys
            WHERE key_hash = $1
            "#,
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row_opt {
            Ok(Some(ApiKey {
                id: ApiKeyId(row.try_get("id")?),
                tap_id: TapId(row.try_get("tap_id")?),
                name: row.try_get("name")?,
                key_hash: row.try_get("key_hash")?,
                scopes: serde_json::from_value(row.try_get("scopes")?)?,
                last_used_at: row.try_get("last_used_at")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, key: &ApiKey) -> CoreResult<ApiKey> {
        let id = key.id.0;
        let name = key.name.clone();
        let key_hash = key.key_hash.clone();
        let scopes = serde_json::to_value(&key.scopes)?;
        let last_used_at = key.last_used_at;

        let row = sqlx::query(
            r#"
            UPDATE api_keys
            SET name = $1, key_hash = $2, scopes = $3, last_used_at = $4
            WHERE id = $5
            RETURNING id, tap_id, name, key_hash, scopes, last_used_at, created_at
            "#,
        )
        .bind(name)
        .bind(key_hash)
        .bind(scopes)
        .bind(last_used_at)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(ApiKey {
            id: ApiKeyId(row.try_get("id")?),
            tap_id: TapId(row.try_get("tap_id")?),
            name: row.try_get("name")?,
            key_hash: row.try_get("key_hash")?,
            scopes: serde_json::from_value(row.try_get("scopes")?)?,
            last_used_at: row.try_get("last_used_at")?,
            created_at: row.try_get("created_at")?,
        })
    }

    async fn delete(&self, id: Uuid) -> CoreResult<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
