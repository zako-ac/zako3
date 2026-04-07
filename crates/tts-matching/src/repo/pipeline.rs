use async_trait::async_trait;

use crate::{db::Db, repo::PipelineRepository, Result};

#[derive(Clone)]
pub struct SqlitePipelineRepository {
    db: Db,
}

impl SqlitePipelineRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PipelineRepository for SqlitePipelineRepository {
    async fn get_ordered(&self) -> Result<Vec<String>> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn();
            let mut stmt =
                conn.prepare("SELECT mapper_id FROM pipeline_order ORDER BY position ASC")?;

            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

            let mut mapper_ids = Vec::new();
            for row in rows {
                mapper_ids.push(row?);
            }
            Ok(mapper_ids)
        })
        .await?
    }

    async fn set_ordered(&self, mapper_ids: &[String]) -> Result<()> {
        let db = self.db.clone();
        let mapper_ids = mapper_ids.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn();

            // Delete all existing entries
            conn.execute("DELETE FROM pipeline_order", [])?;

            // Insert new entries with their positions
            for (position, mapper_id) in mapper_ids.iter().enumerate() {
                conn.execute(
                    "INSERT INTO pipeline_order (position, mapper_id) VALUES (?1, ?2)",
                    rusqlite::params![position as i64, mapper_id],
                )?;
            }

            Ok(())
        })
        .await?
    }
}
