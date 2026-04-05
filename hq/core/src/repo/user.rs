use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{DiscordUserId, User, UserId, Username};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_discord_id(&self, discord_id: &str) -> CoreResult<Option<User>>;
    async fn create(&self, user: &User) -> CoreResult<User>;
    async fn find_by_id(&self, id: Uuid) -> CoreResult<Option<User>>;
}

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn find_by_discord_id(&self, discord_id: &str) -> CoreResult<Option<User>> {
        let row = sqlx::query(
            "SELECT id, discord_user_id, username, avatar_url, email, permissions, created_at, updated_at FROM users WHERE discord_user_id = $1",
        )
        .bind(discord_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let id: Uuid = row.try_get("id")?;
            let discord_user_id: String = row.try_get("discord_user_id")?;
            let username: String = row.try_get("username")?;
            let avatar_url: Option<String> = row.try_get("avatar_url")?;
            let email: Option<String> = row.try_get("email")?;
            let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            Ok(Some(User {
                id: UserId(id),
                discord_user_id: DiscordUserId(discord_user_id),
                username: Username(username),
                avatar_url,
                email,
                permissions,
                timestamp: hq_types::hq::ResourceTimestamp {
                    created_at,
                    updated_at,
                },
            }))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, user: &User) -> CoreResult<User> {
        let id: Uuid = user.id.0;
        let discord_id: String = user.discord_user_id.clone().into();
        let username: String = user.username.clone().into();
        let avatar_url = user.avatar_url.clone();
        let email = user.email.clone();
        let permissions = user.permissions.clone();
        let created_at = user.timestamp.created_at;
        let updated_at = user.timestamp.updated_at;

        sqlx::query(
            r#"
            INSERT INTO users (id, discord_user_id, username, avatar_url, email, permissions, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(discord_id)
        .bind(username)
        .bind(avatar_url)
        .bind(email)
        .bind(permissions)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;

        Ok(user.clone())
    }

    async fn find_by_id(&self, id: Uuid) -> CoreResult<Option<User>> {
        let row = sqlx::query("SELECT id, discord_user_id, username, avatar_url, email, permissions, created_at, updated_at FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let id: Uuid = row.try_get("id")?;
            let discord_user_id: String = row.try_get("discord_user_id")?;
            let username: String = row.try_get("username")?;
            let avatar_url: Option<String> = row.try_get("avatar_url")?;
            let email: Option<String> = row.try_get("email")?;
            let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            Ok(Some(User {
                id: UserId(id),
                discord_user_id: DiscordUserId(discord_user_id),
                username: Username(username),
                avatar_url,
                email,
                permissions,
                timestamp: hq_types::hq::ResourceTimestamp {
                    created_at,
                    updated_at,
                },
            }))
        } else {
            Ok(None)
        }
    }
}
