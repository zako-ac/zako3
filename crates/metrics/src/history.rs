use async_trait::async_trait;
use sqlx::PgPool;
use zako3_types::hq::{history::PlayAudioHistory, next_id};
use crate::error::Result;

#[async_trait]
pub trait UseHistoryRepository: Send + Sync {
    async fn insert(&self, entry: &PlayAudioHistory) -> Result<()>;
}

#[derive(Clone)]
pub struct PgUseHistoryRepository {
    pool: PgPool,
}

impl PgUseHistoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UseHistoryRepository for PgUseHistoryRepository {
    async fn insert(&self, entry: &PlayAudioHistory) -> Result<()> {
        let id = next_id() as i64;
        let user_id = entry.user_id.as_ref().map(|u| u.0.clone());
        let discord_user_id = entry.discord_user_id.as_ref().map(|u| u.0.clone());
        let tap_id = entry.tap_id.0.clone();
        let ars_length = entry.ars_length as i32;

        sqlx::query(
            r#"INSERT INTO use_history (id, tap_id, user_id, discord_user_id, ars_length, trace_id, cache_hit, success)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (trace_id) DO NOTHING"#,
        )
        .bind(id)
        .bind(&tap_id)
        .bind(&user_id)
        .bind(&discord_user_id)
        .bind(ars_length)
        .bind(&entry.trace_id)
        .bind(entry.cache_hit)
        .bind(entry.success)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
