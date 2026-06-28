use async_trait::async_trait;
use sqlx::{PgPool, Row};
use zako3_tts_matching::{Error as TtsError, PipelineRepository, Result as TtsResult};

fn db_err(e: sqlx::Error) -> TtsError {
    TtsError::Repository(e.to_string())
}

pub struct PgPipelineRepository {
    pool: PgPool,
}

impl PgPipelineRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineRepository for PgPipelineRepository {
    async fn get_ordered(&self) -> TtsResult<Vec<String>> {
        let rows = sqlx::query("SELECT mapper_id FROM pipeline_order ORDER BY position ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            ids.push(row.try_get::<String, _>("mapper_id").map_err(db_err)?);
        }
        Ok(ids)
    }

    async fn set_ordered(&self, mapper_ids: &[String]) -> TtsResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;

        sqlx::query("DELETE FROM pipeline_order")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

        for (position, mapper_id) in mapper_ids.iter().enumerate() {
            sqlx::query("INSERT INTO pipeline_order (position, mapper_id) VALUES ($1, $2)")
                .bind(position as i32)
                .bind(mapper_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }

        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
