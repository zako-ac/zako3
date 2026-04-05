use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{TapId, UserId, VerificationRequest, VerificationRequestId, VerificationStatus};
use sqlx::{PgPool, Row};

#[async_trait]
pub trait VerificationRepository: Send + Sync {
    async fn create(&self, request: &VerificationRequest) -> CoreResult<VerificationRequest>;
    async fn find_by_id(&self, id: u64) -> CoreResult<Option<VerificationRequest>>;
    async fn list_all(
        &self,
        status: Option<VerificationStatus>,
        page: u32,
        per_page: u32,
    ) -> CoreResult<(Vec<VerificationRequest>, u64)>;
    async fn update_status(
        &self,
        id: u64,
        status: VerificationStatus,
        rejection_reason: Option<String>,
    ) -> CoreResult<VerificationRequest>;
    async fn find_pending_by_tap_id(&self, tap_id: u64) -> CoreResult<Option<VerificationRequest>>;
}

pub struct PgVerificationRepository {
    pool: PgPool,
}

impl PgVerificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VerificationRepository for PgVerificationRepository {
    async fn create(&self, request: &VerificationRequest) -> CoreResult<VerificationRequest> {
        let id = request.id.0 as i64;
        let tap_id = request.tap_id.0 as i64;
        let requester_id = request.requester_id.0 as i64;
        let status = serde_json::to_string(&request.status)?
            .trim_matches('"')
            .to_string();

        sqlx::query(
            r#"
            INSERT INTO verification_requests (id, tap_id, requester_id, title, description, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(tap_id)
        .bind(requester_id)
        .bind(&request.title)
        .bind(&request.description)
        .bind(status)
        .bind(request.created_at)
        .bind(request.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(request.clone())
    }

    async fn find_by_id(&self, id: u64) -> CoreResult<Option<VerificationRequest>> {
        let row = sqlx::query(
            r#"
            SELECT id, tap_id, requester_id, title, description, status, rejection_reason, created_at, updated_at
            FROM verification_requests
            WHERE id = $1
            "#,
        )
        .bind(id as i64)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(self.row_to_request(row)?))
        } else {
            Ok(None)
        }
    }

    async fn list_all(
        &self,
        status: Option<VerificationStatus>,
        page: u32,
        per_page: u32,
    ) -> CoreResult<(Vec<VerificationRequest>, u64)> {
        let offset = ((page.max(1) - 1) * per_page) as i64;
        let limit = per_page as i64;
        let status_str = status.map(|s| {
            serde_json::to_string(&s)
                .unwrap()
                .trim_matches('"')
                .to_string()
        });

        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT id, tap_id, requester_id, title, description, status, rejection_reason, created_at, updated_at FROM verification_requests ",
        );

        if let Some(ref s) = status_str {
            query_builder.push("WHERE status = ");
            query_builder.push_bind(s);
        }

        query_builder.push(" ORDER BY created_at DESC LIMIT ");
        query_builder.push_bind(limit);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset);

        let rows = query_builder.build().fetch_all(&self.pool).await?;

        let mut count_builder =
            sqlx::QueryBuilder::new("SELECT COUNT(*) FROM verification_requests ");
        if let Some(ref s) = status_str {
            count_builder.push("WHERE status = ");
            count_builder.push_bind(s);
        }
        let total: i64 = count_builder.build().fetch_one(&self.pool).await?.get(0);

        let requests = rows
            .into_iter()
            .map(|r| self.row_to_request(r))
            .collect::<CoreResult<Vec<_>>>()?;

        Ok((requests, total as u64))
    }

    async fn update_status(
        &self,
        id: u64,
        status: VerificationStatus,
        rejection_reason: Option<String>,
    ) -> CoreResult<VerificationRequest> {
        let status_str = serde_json::to_string(&status)?
            .trim_matches('"')
            .to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            UPDATE verification_requests
            SET status = $1, rejection_reason = $2, updated_at = $3
            WHERE id = $4
            "#,
        )
        .bind(status_str)
        .bind(rejection_reason)
        .bind(now)
        .bind(id as i64)
        .execute(&self.pool)
        .await?;

        let updated = self.find_by_id(id).await?.ok_or_else(|| {
            crate::CoreError::NotFound("Verification request not found after update".to_string())
        })?;
        Ok(updated)
    }

    async fn find_pending_by_tap_id(&self, tap_id: u64) -> CoreResult<Option<VerificationRequest>> {
        let row = sqlx::query(
            r#"
            SELECT id, tap_id, requester_id, title, description, status, rejection_reason, created_at, updated_at
            FROM verification_requests
            WHERE tap_id = $1 AND status = 'pending'
            "#,
        )
        .bind(tap_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(self.row_to_request(row)?))
        } else {
            Ok(None)
        }
    }
}

impl PgVerificationRepository {
    fn row_to_request(&self, row: sqlx::postgres::PgRow) -> CoreResult<VerificationRequest> {
        let id: i64 = row.try_get("id")?;
        let tap_id: i64 = row.try_get("tap_id")?;
        let requester_id: i64 = row.try_get("requester_id")?;
        let title: String = row.try_get("title")?;
        let description: String = row.try_get("description")?;
        let status_str: String = row.try_get("status")?;
        let status: VerificationStatus = serde_json::from_str(&format!("\"{}\"", status_str))?;
        let rejection_reason: Option<String> = row.try_get("rejection_reason")?;
        let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
        let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

        Ok(VerificationRequest {
            id: VerificationRequestId(id as u64),
            tap_id: TapId(tap_id as u64),
            requester_id: UserId(requester_id as u64),
            title,
            description,
            status,
            rejection_reason,
            created_at,
            updated_at,
        })
    }
}
