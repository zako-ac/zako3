use async_trait::async_trait;
use hq_types::hq::settings::{EmojiMappingRule, PartialUserSettings, UserSettingsField};
use pgvector::Vector;
use sqlx::{PgPool, Row};
use std::sync::Arc;

use crate::types::{ImgHash, Scope};

pub type ArcHashCache = Arc<dyn HashCache + Send + Sync>;
pub type ArcSettingsStore = Arc<dyn SettingsStore + Send + Sync>;

#[async_trait]
pub trait HashCache {
    async fn get(&self, emoji_id: &str) -> anyhow::Result<Option<ImgHash>>;
    async fn put(&self, emoji_id: &str, hash: &ImgHash) -> anyhow::Result<()>;
}

#[async_trait]
pub trait SettingsStore {
    async fn read_scope(&self, scope: &Scope) -> anyhow::Result<Option<PartialUserSettings>>;
    async fn append_emoji_rule(&self, scope: &Scope, rule: EmojiMappingRule) -> anyhow::Result<()>;
}

pub struct PgHashCache {
    pool: PgPool,
}

impl PgHashCache {
    pub async fn new(pool: PgPool) -> anyhow::Result<Self> {
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS emoji_hash_cache (
                emoji_id TEXT PRIMARY KEY,
                embedding vector(64) NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl HashCache for PgHashCache {
    async fn get(&self, emoji_id: &str) -> anyhow::Result<Option<ImgHash>> {
        let row: Option<(Vector,)> =
            sqlx::query_as("SELECT embedding FROM emoji_hash_cache WHERE emoji_id = $1")
                .bind(emoji_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(v,)| ImgHash::from_float_vec(v.to_vec())))
    }

    async fn put(&self, emoji_id: &str, hash: &ImgHash) -> anyhow::Result<()> {
        let embedding = Vector::from(hash.to_float_vec());
        sqlx::query(
            "INSERT INTO emoji_hash_cache (emoji_id, embedding) VALUES ($1, $2) \
             ON CONFLICT (emoji_id) DO UPDATE SET embedding = EXCLUDED.embedding, updated_at = now()",
        )
        .bind(emoji_id)
        .bind(embedding)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

pub struct PgSettingsStore {
    pool: PgPool,
}

impl PgSettingsStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn parse_settings(value: serde_json::Value) -> Option<PartialUserSettings> {
    serde_json::from_value(value).ok()
}

#[async_trait]
impl SettingsStore for PgSettingsStore {
    async fn read_scope(&self, scope: &Scope) -> anyhow::Result<Option<PartialUserSettings>> {
        let value: Option<serde_json::Value> = match scope {
            Scope::Global => {
                sqlx::query("SELECT settings FROM global_settings WHERE id = 1")
                    .fetch_optional(&self.pool)
                    .await?
                    .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                    .flatten()
            }
            Scope::Guild(gid) => {
                sqlx::query("SELECT settings FROM guild_settings WHERE guild_id = $1")
                    .bind(gid)
                    .fetch_optional(&self.pool)
                    .await?
                    .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                    .flatten()
            }
            Scope::User(uid) => {
                sqlx::query("SELECT settings FROM users WHERE id = $1")
                    .bind(uid)
                    .fetch_optional(&self.pool)
                    .await?
                    .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                    .flatten()
            }
            Scope::GuildUser { user_id, guild_id } => {
                sqlx::query(
                    "SELECT settings FROM user_guild_settings WHERE user_id = $1 AND guild_id = $2",
                )
                .bind(user_id)
                .bind(guild_id)
                .fetch_optional(&self.pool)
                .await?
                .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                .flatten()
            }
        };

        Ok(value.and_then(parse_settings))
    }

    async fn append_emoji_rule(
        &self,
        scope: &Scope,
        rule: EmojiMappingRule,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        let current_value: Option<serde_json::Value> = match scope {
            Scope::Global => sqlx::query("SELECT settings FROM global_settings WHERE id = 1 FOR UPDATE")
                .fetch_optional(&mut *tx)
                .await?
                .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                .flatten(),
            Scope::Guild(gid) => {
                sqlx::query("SELECT settings FROM guild_settings WHERE guild_id = $1 FOR UPDATE")
                    .bind(gid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                    .flatten()
            }
            Scope::User(uid) => {
                sqlx::query("SELECT settings FROM users WHERE id = $1 FOR UPDATE")
                    .bind(uid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
                    .flatten()
            }
            Scope::GuildUser { user_id, guild_id } => sqlx::query(
                "SELECT settings FROM user_guild_settings WHERE user_id = $1 AND guild_id = $2 FOR UPDATE",
            )
            .bind(user_id)
            .bind(guild_id)
            .fetch_optional(&mut *tx)
            .await?
            .and_then(|r| r.try_get::<Option<serde_json::Value>, _>("settings").ok())
            .flatten(),
        };

        let mut settings: PartialUserSettings = current_value
            .and_then(parse_settings)
            .unwrap_or_else(PartialUserSettings::empty);

        let (mut rules, important) = match std::mem::take(&mut settings.emoji_mappings) {
            UserSettingsField::None => (Vec::new(), false),
            UserSettingsField::Normal(v) => (v, false),
            UserSettingsField::Important(v) => (v, true),
        };

        if rules.iter().any(|r| r.emoji_id == rule.emoji_id) {
            tracing::debug!(
                emoji_id = %rule.emoji_id,
                "skipping append: rule already present in scope"
            );
            return Ok(());
        }

        rules.push(rule);

        settings.emoji_mappings = if important {
            UserSettingsField::Important(rules)
        } else {
            UserSettingsField::Normal(rules)
        };

        let json = serde_json::to_value(&settings)?;

        match scope {
            Scope::Global => {
                sqlx::query(
                    "INSERT INTO global_settings (id, settings) VALUES (1, $1) \
                     ON CONFLICT (id) DO UPDATE SET settings = EXCLUDED.settings, updated_at = now()",
                )
                .bind(json)
                .execute(&mut *tx)
                .await?;
            }
            Scope::Guild(gid) => {
                sqlx::query(
                    "INSERT INTO guild_settings (guild_id, settings) VALUES ($1, $2) \
                     ON CONFLICT (guild_id) DO UPDATE SET settings = EXCLUDED.settings, updated_at = now()",
                )
                .bind(gid)
                .bind(json)
                .execute(&mut *tx)
                .await?;
            }
            Scope::User(uid) => {
                sqlx::query("UPDATE users SET settings = $1, updated_at = now() WHERE id = $2")
                    .bind(json)
                    .bind(uid)
                    .execute(&mut *tx)
                    .await?;
            }
            Scope::GuildUser { user_id, guild_id } => {
                sqlx::query(
                    "INSERT INTO user_guild_settings (user_id, guild_id, settings) VALUES ($1, $2, $3) \
                     ON CONFLICT (user_id, guild_id) DO UPDATE SET settings = EXCLUDED.settings, updated_at = now()",
                )
                .bind(user_id)
                .bind(guild_id)
                .bind(json)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }
}
