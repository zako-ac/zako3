use crate::CoreResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackAction {
    pub id: String,
    pub action_type: String,
    pub guild_id: String,
    pub channel_id: String,
    pub actor_discord_user_id: String,
    pub track_snapshot: serde_json::Value,
    pub queue_snapshot: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub undone_at: Option<DateTime<Utc>>,
    pub undone_by_discord_user_id: Option<String>,
}

pub struct CreatePlaybackAction {
    pub action_type: String,
    pub guild_id: String,
    pub channel_id: String,
    pub actor_discord_user_id: String,
    pub track_snapshot: serde_json::Value,
    pub queue_snapshot: Option<serde_json::Value>,
}

#[async_trait]
pub trait PlaybackActionRepo: Send + Sync {
    async fn create(&self, dto: &CreatePlaybackAction) -> CoreResult<PlaybackAction>;
    async fn find_by_id(&self, id: &str) -> CoreResult<Option<PlaybackAction>>;
    async fn find_by_guild_ids(
        &self,
        guild_ids: &[String],
        limit: i64,
    ) -> CoreResult<Vec<PlaybackAction>>;
    async fn mark_undone(
        &self,
        id: &str,
        undone_by: &str,
        undone_at: DateTime<Utc>,
    ) -> CoreResult<PlaybackAction>;
}

pub struct PgPlaybackActionRepo {
    pool: PgPool,
}

impl PgPlaybackActionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn row_to_action(row: &sqlx::postgres::PgRow) -> CoreResult<PlaybackAction> {
    Ok(PlaybackAction {
        id: row.try_get("id")?,
        action_type: row.try_get("action_type")?,
        guild_id: row.try_get("guild_id")?,
        channel_id: row.try_get("channel_id")?,
        actor_discord_user_id: row.try_get("actor_discord_user_id")?,
        track_snapshot: row.try_get("track_snapshot")?,
        queue_snapshot: row.try_get("queue_snapshot")?,
        created_at: row.try_get("created_at")?,
        undone_at: row.try_get("undone_at")?,
        undone_by_discord_user_id: row.try_get("undone_by_discord_user_id")?,
    })
}

#[async_trait]
impl PlaybackActionRepo for PgPlaybackActionRepo {
    async fn create(&self, dto: &CreatePlaybackAction) -> CoreResult<PlaybackAction> {
        let id = hq_types::hq::next_id().to_string();
        let row = sqlx::query(
            r#"
            INSERT INTO playback_actions
                (id, action_type, guild_id, channel_id, actor_discord_user_id, track_snapshot, queue_snapshot)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, action_type, guild_id, channel_id, actor_discord_user_id,
                      track_snapshot, queue_snapshot, created_at, undone_at, undone_by_discord_user_id
            "#,
        )
        .bind(&id)
        .bind(&dto.action_type)
        .bind(&dto.guild_id)
        .bind(&dto.channel_id)
        .bind(&dto.actor_discord_user_id)
        .bind(&dto.track_snapshot)
        .bind(&dto.queue_snapshot)
        .fetch_one(&self.pool)
        .await?;

        row_to_action(&row)
    }

    async fn find_by_id(&self, id: &str) -> CoreResult<Option<PlaybackAction>> {
        let row = sqlx::query(
            r#"
            SELECT id, action_type, guild_id, channel_id, actor_discord_user_id,
                   track_snapshot, queue_snapshot, created_at, undone_at, undone_by_discord_user_id
            FROM playback_actions WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_action(&r)).transpose()
    }

    async fn find_by_guild_ids(
        &self,
        guild_ids: &[String],
        limit: i64,
    ) -> CoreResult<Vec<PlaybackAction>> {
        let rows = sqlx::query(
            r#"
            SELECT id, action_type, guild_id, channel_id, actor_discord_user_id,
                   track_snapshot, queue_snapshot, created_at, undone_at, undone_by_discord_user_id
            FROM playback_actions
            WHERE guild_id = ANY($1)
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(guild_ids)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_action).collect()
    }

    async fn mark_undone(
        &self,
        id: &str,
        undone_by: &str,
        undone_at: DateTime<Utc>,
    ) -> CoreResult<PlaybackAction> {
        let row = sqlx::query(
            r#"
            UPDATE playback_actions
            SET undone_at = $1, undone_by_discord_user_id = $2
            WHERE id = $3
            RETURNING id, action_type, guild_id, channel_id, actor_discord_user_id,
                      track_snapshot, queue_snapshot, created_at, undone_at, undone_by_discord_user_id
            "#,
        )
        .bind(undone_at)
        .bind(undone_by)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        row_to_action(&row)
    }
}
