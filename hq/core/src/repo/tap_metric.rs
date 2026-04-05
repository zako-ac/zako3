use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct TapMetricRepository {
    pool: PgPool,
}

impl TapMetricRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record_metric(&self, tap_id: Uuid, metric_type: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tap_metrics (tap_id, metric_type)
            VALUES ($1, $2)
            "#,
        )
        .bind(tap_id)
        .bind(metric_type)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_total_uses(&self, tap_id: Uuid) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM tap_metrics
            WHERE tap_id = $1 AND metric_type = 'request'
            "#,
        )
        .bind(tap_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }
}
