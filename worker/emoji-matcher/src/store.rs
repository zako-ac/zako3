use pgvector::Vector;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::types::{Emoji, ImgHash};

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct MockEmojiStore;

#[cfg(test)]
impl MockEmojiStore {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl EmojiStore for MockEmojiStore {
    async fn save_emoji(&self, _emoji: Emoji) -> anyhow::Result<()> {
        Ok(())
    }
    async fn find_similar_emoji(&self, _hash: &ImgHash) -> anyhow::Result<Option<Emoji>> {
        Ok(None)
    }
}

pub type ArcEmojiStore = Arc<dyn EmojiStore + Send + Sync>;

#[async_trait::async_trait]
pub trait EmojiStore {
    async fn save_emoji(&self, emoji: Emoji) -> anyhow::Result<()>;
    async fn find_similar_emoji(&self, hash: &ImgHash) -> anyhow::Result<Option<Emoji>>;
}

pub struct PgEmojiStore {
    pool: Pool<Postgres>,
}

impl PgEmojiStore {
    pub async fn new(db_url: &str) -> anyhow::Result<Self> {
        let pool = Pool::<Postgres>::connect(db_url).await?;

        // Ensure extension exists
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&pool)
            .await?;

        // Ensure table exists
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS emojis (
                id SERIAL PRIMARY KEY,
                registered_id TEXT NOT NULL,
                name TEXT NOT NULL,
                embedding vector(64) NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        // Create HNSW index for faster similarity search
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS emojis_embedding_idx ON emojis USING hnsw (embedding vector_l2_ops)",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl EmojiStore for PgEmojiStore {
    async fn save_emoji(&self, emoji: Emoji) -> anyhow::Result<()> {
        let embedding = Vector::from(emoji.hash.to_float_vec());

        sqlx::query("INSERT INTO emojis (registered_id, name, embedding) VALUES ($1, $2, $3)")
            .bind(emoji.registered_id)
            .bind(emoji.name)
            .bind(embedding)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn find_similar_emoji(&self, hash: &ImgHash) -> anyhow::Result<Option<Emoji>> {
        let row: Option<(String, String, Vector, f64)> = sqlx::query_as(
            "SELECT registered_id, name, embedding, embedding <-> $1 as dist 
             FROM emojis 
             ORDER BY dist 
             LIMIT 1",
        )
        .bind(Vector::from(hash.to_float_vec()))
        .fetch_optional(&self.pool)
        .await?;

        if let Some((registered_id, name, stored_vec, dist)) = row {
            // Threshold: Hamming distance of 5 corresponds to squared L2 distance of 5.
            // The <-> operator returns Euclidean distance (L2 distance).
            // So we check if dist < sqrt(5).
            if dist < (5.0f64).sqrt() {
                return Ok(Some(Emoji {
                    hash: ImgHash::from_float_vec(stored_vec.to_vec()),
                    registered_id,
                    name,
                }));
            }
        }

        Ok(None)
    }
}
