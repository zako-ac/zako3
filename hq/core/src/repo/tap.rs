use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{Tap, TapId, TapName, UserId};
use sqlx::{PgPool, Row};

#[async_trait]
pub trait TapRepository: Send + Sync {
    async fn create(&self, tap: &Tap) -> CoreResult<Tap>;
    async fn list_by_owner(&self, owner_id: u64) -> CoreResult<Vec<Tap>>;
    async fn find_by_id(&self, id: u64) -> CoreResult<Option<Tap>>;
    async fn update(&self, tap: &Tap) -> CoreResult<Tap>;
    async fn delete(&self, id: u64) -> CoreResult<()>;
}

pub struct PgTapRepository {
    pool: PgPool,
}

impl PgTapRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TapRepository for PgTapRepository {
    async fn create(&self, tap: &Tap) -> CoreResult<Tap> {
        let id = tap.id.0 as i64;
        let owner_id = tap.owner_id.0 as i64;
        let name = tap.name.0.clone();
        let description = tap.description.clone();
        let occupation = serde_json::to_string(&tap.occupation)?
            .trim_matches('"')
            .to_string();
        let permission = serde_json::to_value(&tap.permission)?;
        let roles = serde_json::to_value(&tap.roles)?;

        sqlx::query(
            r#"
            INSERT INTO taps (id, owner_id, name, description, occupation, permission, roles, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(owner_id)
        .bind(name)
        .bind(description)
        .bind(occupation)
        .bind(permission)
        .bind(roles)
        .bind(tap.timestamp.created_at)
        .bind(tap.timestamp.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(tap.clone())
    }

    async fn list_by_owner(&self, owner_id: u64) -> CoreResult<Vec<Tap>> {
        let rows = sqlx::query(
            r#"
            SELECT id, owner_id, name, description, occupation, permission, roles, created_at, updated_at
            FROM taps
            WHERE owner_id = $1
            "#,
        )
        .bind(owner_id as i64)
        .fetch_all(&self.pool)
        .await?;

        let taps = rows
            .into_iter()
            .map(|row| {
                let id: i64 = row.try_get("id")?;
                let id = id as u64;
                let owner_id: i64 = row.try_get("owner_id")?;
                let owner_id = owner_id as u64;
                let name: String = row.try_get("name")?;
                let description: Option<String> = row.try_get("description")?;

                let occupation_str: String = row.try_get("occupation")?;
                let occupation = serde_json::from_str(&format!("\"{}\"", occupation_str))?;

                let permission_val: serde_json::Value = row.try_get("permission")?;
                let permission = serde_json::from_value(permission_val)?;

                let roles_val: serde_json::Value = row.try_get("roles")?;
                let roles = serde_json::from_value(roles_val)?;

                let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
                let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

                Ok(Tap {
                    id: TapId(id),
                    name: TapName(name),
                    description,
                    owner_id: UserId(owner_id),
                    occupation,
                    permission,
                    roles,
                    timestamp: hq_types::hq::ResourceTimestamp {
                        created_at,
                        updated_at,
                    },
                })
            })
            .collect::<CoreResult<Vec<Tap>>>()?;

        Ok(taps)
    }

    async fn find_by_id(&self, id: u64) -> CoreResult<Option<Tap>> {
        let row = sqlx::query(
            r#"
            SELECT id, owner_id, name, description, occupation, permission, roles, created_at, updated_at
            FROM taps
            WHERE id = $1
            "#,
        )
        .bind(id as i64)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let id: i64 = row.try_get("id")?;
            let id = id as u64;
            let owner_id: i64 = row.try_get("owner_id")?;
            let owner_id = owner_id as u64;
            let name: String = row.try_get("name")?;
            let description: Option<String> = row.try_get("description")?;

            let occupation_str: String = row.try_get("occupation")?;
            let occupation = serde_json::from_str(&format!("\"{}\"", occupation_str))?;

            let permission_val: serde_json::Value = row.try_get("permission")?;
            let permission = serde_json::from_value(permission_val)?;

            let roles_val: serde_json::Value = row.try_get("roles")?;
            let roles = serde_json::from_value(roles_val)?;

            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            Ok(Some(Tap {
                id: TapId(id),
                name: TapName(name),
                description,
                owner_id: UserId(owner_id),
                occupation,
                permission,
                roles,
                timestamp: hq_types::hq::ResourceTimestamp {
                    created_at,
                    updated_at,
                },
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, tap: &Tap) -> CoreResult<Tap> {
        let id = tap.id.0 as i64;
        let name = tap.name.0.clone();
        let description = tap.description.clone();
        let occupation = serde_json::to_string(&tap.occupation)?
            .trim_matches('"')
            .to_string();
        let permission = serde_json::to_value(&tap.permission)?;
        let roles = serde_json::to_value(&tap.roles)?;

        sqlx::query(
            r#"
            UPDATE taps
            SET name = $1, description = $2, occupation = $3, permission = $4, roles = $5, updated_at = $6
            WHERE id = $7
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(occupation)
        .bind(permission)
        .bind(roles)
        .bind(tap.timestamp.updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(tap.clone())
    }

    async fn delete(&self, id: u64) -> CoreResult<()> {
        sqlx::query("DELETE FROM taps WHERE id = $1")
            .bind(id as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
