use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{Notification, NotificationId, UserId};
use sqlx::{PgPool, Row};

#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn create(&self, notification: &Notification) -> CoreResult<Notification>;
    async fn list_by_user(&self, user_id: u64) -> CoreResult<Vec<Notification>>;
    async fn mark_as_read(&self, id: u64, user_id: u64) -> CoreResult<Option<Notification>>;
}

pub struct PgNotificationRepository {
    pool: PgPool,
}

impl PgNotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationRepository for PgNotificationRepository {
    async fn create(&self, n: &Notification) -> CoreResult<Notification> {
        sqlx::query(
            r#"
            INSERT INTO notifications (id, user_id, type, title, message, read_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(n.id.0 as i64)
        .bind(n.user_id.0 as i64)
        .bind(&n.r#type)
        .bind(&n.title)
        .bind(&n.message)
        .bind(n.read_at)
        .bind(n.created_at)
        .execute(&self.pool)
        .await?;

        Ok(n.clone())
    }

    async fn list_by_user(&self, user_id: u64) -> CoreResult<Vec<Notification>> {
        let rows = sqlx::query(
            "SELECT id, user_id, type, title, message, read_at, created_at FROM notifications WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut notifications = Vec::new();
        for row in rows {
            notifications.push(Notification {
                id: NotificationId(row.try_get::<i64, _>("id")? as u64),
                user_id: UserId(row.try_get::<i64, _>("user_id")? as u64),
                r#type: row.try_get("type")?,
                title: row.try_get("title")?,
                message: row.try_get("message")?,
                read_at: row.try_get("read_at")?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(notifications)
    }

    async fn mark_as_read(&self, id: u64, user_id: u64) -> CoreResult<Option<Notification>> {
        let now = chrono::Utc::now();
        let row = sqlx::query(
            r#"
            UPDATE notifications
            SET read_at = $1
            WHERE id = $2 AND user_id = $3
            RETURNING id, user_id, type, title, message, read_at, created_at
            "#,
        )
        .bind(now)
        .bind(id as i64)
        .bind(user_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Notification {
                id: NotificationId(row.try_get::<i64, _>("id")? as u64),
                user_id: UserId(row.try_get::<i64, _>("user_id")? as u64),
                r#type: row.try_get("type")?,
                title: row.try_get("title")?,
                message: row.try_get("message")?,
                read_at: row.try_get("read_at")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }
}
