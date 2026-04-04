use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{Tap, TapId, TapName, UserId};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait TapRepository: Send + Sync {
    async fn create(&self, tap: &Tap) -> CoreResult<Tap>;
    async fn list_by_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Tap>>;
    async fn find_by_id(&self, id: Uuid) -> CoreResult<Option<Tap>>;
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
        let id = tap.id.0;
        let owner_id = tap.owner_id.0;
        let name = tap.name.0.clone();
        let description = tap.description.clone();
        let occupation = serde_json::to_string(&tap.occupation)?
            .trim_matches('"')
            .to_string();
        let permission = serde_json::to_value(&tap.permission)?;
        let role = if let Some(r) = &tap.role {
            Some(serde_json::to_string(r)?.trim_matches('"').to_string())
        } else {
            None
        };

        sqlx::query(
            r#"
            INSERT INTO taps (id, owner_id, name, description, occupation, permission, role, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(owner_id)
        .bind(name)
        .bind(description)
        .bind(occupation)
        .bind(permission)
        .bind(role)
        .bind(tap.timestamp.created_at)
        .bind(tap.timestamp.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(tap.clone())
    }

    async fn list_by_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Tap>> {
        let rows = sqlx::query(
            r#"
            SELECT id, owner_id, name, description, occupation, permission, role, created_at, updated_at
            FROM taps
            WHERE owner_id = $1
            "#,
        )
        .bind(owner_id)
        .fetch_all(&self.pool)
        .await?;

        let taps = rows
            .into_iter()
            .map(|row| {
                let id: Uuid = row.try_get("id")?;
                let owner_id: Uuid = row.try_get("owner_id")?;
                let name: String = row.try_get("name")?;
                let description: Option<String> = row.try_get("description")?;

                let occupation_str: String = row.try_get("occupation")?;
                let occupation = serde_json::from_str(&format!("\"{}\"", occupation_str))?;

                let permission_val: serde_json::Value = row.try_get("permission")?;
                let permission = serde_json::from_value(permission_val)?;

                let role_str: Option<String> = row.try_get("role")?;
                let role = if let Some(r) = role_str {
                    Some(serde_json::from_str(&format!("\"{}\"", r))?)
                } else {
                    None
                };

                let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
                let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

                Ok(Tap {
                    id: TapId(id),
                    name: TapName(name),
                    description,
                    owner_id: UserId(owner_id),
                    occupation,
                    permission,
                    role,
                    timestamp: hq_types::hq::ResourceTimestamp {
                        created_at,
                        updated_at,
                    },
                })
            })
            .collect::<CoreResult<Vec<Tap>>>()?;

        Ok(taps)
    }

    async fn find_by_id(&self, id: Uuid) -> CoreResult<Option<Tap>> {
        let row = sqlx::query(
            r#"
            SELECT id, owner_id, name, description, occupation, permission, role, created_at, updated_at
            FROM taps
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let id: Uuid = row.try_get("id")?;
            let owner_id: Uuid = row.try_get("owner_id")?;
            let name: String = row.try_get("name")?;
            let description: Option<String> = row.try_get("description")?;

            let occupation_str: String = row.try_get("occupation")?;
            let occupation = serde_json::from_str(&format!("\"{}\"", occupation_str))?;

            let permission_val: serde_json::Value = row.try_get("permission")?;
            let permission = serde_json::from_value(permission_val)?;

            let role_str: Option<String> = row.try_get("role")?;
            let role = if let Some(r) = role_str {
                Some(serde_json::from_str(&format!("\"{}\"", r))?)
            } else {
                None
            };

            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            Ok(Some(Tap {
                id: TapId(id),
                name: TapName(name),
                description,
                owner_id: UserId(owner_id),
                occupation,
                permission,
                role,
                timestamp: hq_types::hq::ResourceTimestamp {
                    created_at,
                    updated_at,
                },
            }))
        } else {
            Ok(None)
        }
    }
}
