use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::audit_log::{AuditLog, CreateAuditLogDto};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct AuditLogWithActor {
    pub log: AuditLog,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
}

#[async_trait]
pub trait AuditLogRepo: Send + Sync {
    async fn create(&self, dto: &CreateAuditLogDto) -> CoreResult<AuditLog>;
    async fn find_by_tap_id(
        &self,
        tap_id: String,
        page: i64,
        limit: i64,
    ) -> CoreResult<(Vec<AuditLogWithActor>, i64)>;
}

pub struct PgAuditLogRepo {
    pool: PgPool,
}

impl PgAuditLogRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLogRepo for PgAuditLogRepo {
    async fn create(&self, dto: &CreateAuditLogDto) -> CoreResult<AuditLog> {
        let id = hq_types::hq::next_id().to_string();
        let row = sqlx::query(
            r#"
            INSERT INTO audit_logs (id, tap_id, actor_id, action_type, metadata)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, tap_id, actor_id, action_type, metadata, created_at
            "#,
        )
        .bind(id)
        .bind(&dto.tap_id)
        .bind(&dto.actor_id)
        .bind(&dto.action_type)
        .bind(&dto.metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(AuditLog {
            id: row.try_get("id")?,
            tap_id: row.try_get("tap_id")?,
            actor_id: row.try_get("actor_id")?,
            action_type: row.try_get("action_type")?,
            metadata: row.try_get("metadata")?,
            created_at: row.try_get("created_at")?,
        })
    }

    async fn find_by_tap_id(
        &self,
        tap_id: String,
        page: i64,
        limit: i64,
    ) -> CoreResult<(Vec<AuditLogWithActor>, i64)> {
        let offset = (page - 1) * limit;

        let row = sqlx::query("SELECT COUNT(*) as count FROM audit_logs WHERE tap_id = $1")
            .bind(&tap_id)
            .fetch_one(&self.pool)
            .await?;

        let total: i64 = row.try_get("count").unwrap_or(0);

        let rows = sqlx::query(
            r#"
            SELECT 
                al.id, al.tap_id, al.actor_id, al.action_type, al.metadata, al.created_at,
                u.username, u.avatar_url
            FROM audit_logs al
            LEFT JOIN users u ON al.actor_id = u.id
            WHERE al.tap_id = $1
            ORDER BY al.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&tap_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let records = rows
            .into_iter()
            .map(|row| AuditLogWithActor {
                log: AuditLog {
                    id: row.try_get("id").unwrap_or_default(),
                    tap_id: row.try_get("tap_id").unwrap_or_default(),
                    actor_id: row.try_get("actor_id").ok().flatten(),
                    action_type: row.try_get("action_type").unwrap_or_default(),
                    metadata: row.try_get("metadata").unwrap_or_default(),
                    created_at: row.try_get("created_at").unwrap_or_default(),
                },
                username: row.try_get("username").ok(),
                avatar_url: row.try_get("avatar_url").ok(),
            })
            .collect();

        Ok((records, total))
    }
}
