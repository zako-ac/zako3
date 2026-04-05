use crate::CoreResult;
use async_trait::async_trait;
use hq_types::hq::{DiscordUserId, User, UserId, Username};
use sqlx::{PgPool, Row};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_discord_id(&self, discord_id: &str) -> CoreResult<Option<User>>;
    async fn create(&self, user: &User) -> CoreResult<User>;
    async fn find_by_id(&self, id: UserId) -> CoreResult<Option<User>>;
    async fn list_all(&self, page: u32, per_page: u32) -> CoreResult<(Vec<User>, u64)>;
    async fn update_permissions(&self, id: UserId, permissions: Vec<String>) -> CoreResult<User>;
    async fn set_banned_status(&self, id: UserId, banned: bool) -> CoreResult<User>;
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
            "SELECT id, discord_user_id, username, avatar_url, email, permissions, banned, created_at, updated_at FROM users WHERE discord_user_id = $1",
        )
        .bind(discord_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let id: String = row.try_get("id")?;
            let discord_user_id: String = row.try_get("discord_user_id")?;
            let username: String = row.try_get("username")?;
            let avatar_url: Option<String> = row.try_get("avatar_url")?;
            let email: Option<String> = row.try_get("email")?;
            let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
            let banned: bool = row.try_get("banned")?;
            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            Ok(Some(User {
                id: UserId(id),
                discord_user_id: DiscordUserId(discord_user_id),
                username: Username(username),
                avatar_url,
                email,
                permissions,
                banned,
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
        let id = user.id.0.clone();
        let discord_id: String = user.discord_user_id.clone().into();
        let username: String = user.username.clone().into();
        let avatar_url = user.avatar_url.clone();
        let email = user.email.clone();
        let permissions = user.permissions.clone();
        let banned = user.banned;
        let created_at = user.timestamp.created_at;
        let updated_at = user.timestamp.updated_at;

        sqlx::query(
            r#"
            INSERT INTO users (id, discord_user_id, username, avatar_url, email, permissions, banned, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(discord_id)
        .bind(username)
        .bind(avatar_url)
        .bind(email)
        .bind(permissions)
        .bind(banned)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;

        Ok(user.clone())
    }

    async fn find_by_id(&self, id: UserId) -> CoreResult<Option<User>> {
        let row = sqlx::query("SELECT id, discord_user_id, username, avatar_url, email, permissions, banned, created_at, updated_at FROM users WHERE id = $1")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let id: String = row.try_get("id")?;
            let discord_user_id: String = row.try_get("discord_user_id")?;
            let username: String = row.try_get("username")?;
            let avatar_url: Option<String> = row.try_get("avatar_url")?;
            let email: Option<String> = row.try_get("email")?;
            let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
            let banned: bool = row.try_get("banned")?;
            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            Ok(Some(User {
                id: UserId(id),
                discord_user_id: DiscordUserId(discord_user_id),
                username: Username(username),
                avatar_url,
                email,
                permissions,
                banned,
                timestamp: hq_types::hq::ResourceTimestamp {
                    created_at,
                    updated_at,
                },
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_all(&self, page: u32, per_page: u32) -> CoreResult<(Vec<User>, u64)> {
        let offset = (page - 1) * per_page;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;

        let rows = sqlx::query("SELECT id, discord_user_id, username, avatar_url, email, permissions, banned, created_at, updated_at FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        let mut users = Vec::new();
        for row in rows {
            let id: String = row.try_get("id")?;
            let discord_user_id: String = row.try_get("discord_user_id")?;
            let username: String = row.try_get("username")?;
            let avatar_url: Option<String> = row.try_get("avatar_url")?;
            let email: Option<String> = row.try_get("email")?;
            let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
            let banned: bool = row.try_get("banned")?;
            let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
            let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

            users.push(User {
                id: UserId(id),
                discord_user_id: DiscordUserId(discord_user_id),
                username: Username(username),
                avatar_url,
                email,
                permissions,
                banned,
                timestamp: hq_types::hq::ResourceTimestamp {
                    created_at,
                    updated_at,
                },
            });
        }
        Ok((users, total.0 as u64))
    }

    async fn update_permissions(&self, id: UserId, permissions: Vec<String>) -> CoreResult<User> {
        let row = sqlx::query(
            "UPDATE users SET permissions = $1, updated_at = $2 WHERE id = $3 RETURNING id, discord_user_id, username, avatar_url, email, permissions, banned, created_at, updated_at",
        )
        .bind(&permissions)
        .bind(chrono::Utc::now())
        .bind(id.0)
        .fetch_one(&self.pool)
        .await?;

        let id_val: String = row.try_get("id")?;
        let discord_user_id: String = row.try_get("discord_user_id")?;
        let username: String = row.try_get("username")?;
        let avatar_url: Option<String> = row.try_get("avatar_url")?;
        let email: Option<String> = row.try_get("email")?;
        let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
        let banned: bool = row.try_get("banned")?;
        let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
        let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

        Ok(User {
            id: UserId(id_val),
            discord_user_id: DiscordUserId(discord_user_id),
            username: Username(username),
            avatar_url,
            email,
            permissions,
            banned,
            timestamp: hq_types::hq::ResourceTimestamp {
                created_at,
                updated_at,
            },
        })
    }

    async fn set_banned_status(&self, id: UserId, banned: bool) -> CoreResult<User> {
        let row = sqlx::query(
            "UPDATE users SET banned = $1, updated_at = $2 WHERE id = $3 RETURNING id, discord_user_id, username, avatar_url, email, permissions, banned, created_at, updated_at",
        )
        .bind(banned)
        .bind(chrono::Utc::now())
        .bind(id.0)
        .fetch_one(&self.pool)
        .await?;

        let id_val: String = row.try_get("id")?;
        let discord_user_id: String = row.try_get("discord_user_id")?;
        let username: String = row.try_get("username")?;
        let avatar_url: Option<String> = row.try_get("avatar_url")?;
        let email: Option<String> = row.try_get("email")?;
        let permissions: Vec<String> = row.try_get("permissions").unwrap_or_default();
        let banned: bool = row.try_get("banned")?;
        let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
        let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

        Ok(User {
            id: UserId(id_val),
            discord_user_id: DiscordUserId(discord_user_id),
            username: Username(username),
            avatar_url,
            email,
            permissions,
            banned,
            timestamp: hq_types::hq::ResourceTimestamp {
                created_at,
                updated_at,
            },
        })
    }
}
