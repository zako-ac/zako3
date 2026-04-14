use crate::repo::UserRepository;
use crate::service::GuildInfo;
use crate::{AppConfig, CoreError, CoreResult};
use hq_types::hq::{AuthResponseDto, AuthUserDto, User, UserId};
use jsonwebtoken::{EncodingKey, Header, encode};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // internal user id
    pub exp: usize,
}

#[derive(Clone)]
pub struct AuthService {
    config: Arc<AppConfig>,
    user_repo: Arc<dyn UserRepository>,
    client: Client,
}

impl AuthService {
    pub fn new(config: Arc<AppConfig>, user_repo: Arc<dyn UserRepository>) -> Self {
        Self {
            config,
            user_repo,
            client: Client::new(),
        }
    }

    pub fn get_login_url(&self, redirect: Option<&str>) -> String {
        let mut url = format!(
            "https://discord.com/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify%20email%20guilds",
            self.config.discord_client_id,
            urlencoding::encode(&self.config.discord_redirect_uri)
        );
        if let Some(path) = redirect {
            url.push_str("&state=");
            url.push_str(&urlencoding::encode(path));
        }
        url
    }

    #[tracing::instrument(skip(self, code, state), name = "auth.authenticate", err)]
    pub async fn authenticate(
        &self,
        code: &str,
        state: Option<&str>,
    ) -> CoreResult<AuthResponseDto> {
        // Exchange code for token
        let params = [
            ("client_id", &self.config.discord_client_id),
            ("client_secret", &self.config.discord_client_secret),
            ("grant_type", &"authorization_code".to_string()),
            ("code", &code.to_string()),
            ("redirect_uri", &self.config.discord_redirect_uri),
        ];

        let token_res = self
            .client
            .post("https://discord.com/api/oauth2/token")
            .form(&params)
            .send()
            .await?;

        if !token_res.status().is_success() {
            let status = token_res.status();
            let body = token_res.text().await?;
            return Err(CoreError::Unauthorized(format!(
                "Discord OAuth2 token request failed: {} - {}",
                status, body
            )));
        }

        let token_data: serde_json::Value = token_res.json().await?;

        let access_token = token_data["access_token"]
            .as_str()
            .ok_or(CoreError::Unauthorized("Invalid OAuth2 code".to_string()))?;

        // Get user info
        let user_res = self
            .client
            .get("https://discord.com/api/users/@me")
            .bearer_auth(access_token)
            .send()
            .await?;

        if !user_res.status().is_success() {
            let status = user_res.status();
            let body = user_res.text().await?;
            return Err(CoreError::Unauthorized(format!(
                "Discord user info request failed: {} - {}",
                status, body
            )));
        }

        let user_data: serde_json::Value = user_res.json().await?;

        let discord_id = user_data["id"].as_str().ok_or(CoreError::Unauthorized(
            "Failed to get user info".to_string(),
        ))?;
        let username = user_data["username"]
            .as_str()
            .ok_or(CoreError::Unauthorized(
                "Missing username in Discord response".to_string(),
            ))?;

        let avatar = user_data["avatar"].as_str().map(|s| s.to_string());
        let email = user_data["email"].as_str().map(|s| s.to_string());

        // Find or create user
        let user = self
            .get_or_create_user(discord_id, username, avatar.as_deref(), email.as_deref(), Some(access_token))
            .await?;

        if user.banned {
            return Err(CoreError::Forbidden("User is banned".to_string()));
        }

        // Generate JWT
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::days(7))
            .expect("valid timestamp")
            .timestamp();

        let claims = Claims {
            sub: user.id.0.to_string(),
            exp: expiration as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )?;

        let avatar_url = avatar
            .map(|a| format!("https://cdn.discordapp.com/avatars/{}/{}", discord_id, a))
            .unwrap_or_default();

        let auth_user = AuthUserDto {
            id: user.id.0.to_string(),
            discord_id: user.discord_user_id.0.clone(),
            username: user.username.0.clone(),
            avatar: avatar_url,
            email: user.email.clone(),
            is_admin: user.permissions.contains(&"admin".to_string()),
            banned: user.banned,
        };

        let redirect_url = state
            .and_then(|s| urlencoding::decode(s).ok())
            .map(|s| s.into_owned());

        Ok(AuthResponseDto {
            token,
            user: auth_user,
            redirect_url,
        })
    }

    pub async fn get_or_create_user(
        &self,
        discord_id: &str,
        username: &str,
        avatar: Option<&str>,
        email: Option<&str>,
        oauth_access_token: Option<&str>,
    ) -> CoreResult<User> {
        match self.user_repo.find_by_discord_id(discord_id).await? {
            Some(mut u) => {
                // Update token if provided
                if let Some(token) = oauth_access_token {
                    u.oauth_access_token = Some(token.to_string());
                }
                Ok(u)
            }
            None => {
                let mut new_user = User::new(
                    hq_types::hq::next_id().to_string(),
                    discord_id.to_string(),
                    username.to_string(),
                );

                if let Some(a) = avatar {
                    new_user.avatar_url = Some(format!(
                        "https://cdn.discordapp.com/avatars/{}/{}",
                        discord_id, a
                    ));
                }
                new_user.email = email.map(|s| s.to_string());
                new_user.oauth_access_token = oauth_access_token.map(|s| s.to_string());

                self.user_repo.create(&new_user).await
            }
        }
    }

    #[tracing::instrument(skip(self), name = "auth.get_user", fields(user_id = id), err)]
    pub async fn get_user(&self, id: &str) -> CoreResult<AuthUserDto> {
        let user_id = UserId::from_str(id)
            .map_err(|_| CoreError::InvalidInput("Invalid user ID format".to_string()))?;
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))?;

        Ok(AuthUserDto {
            id: user.id.0.clone(),
            discord_id: user.discord_user_id.0.clone(),
            username: user.username.0.clone(),
            avatar: user.avatar_url.unwrap_or_default(),
            email: user.email.clone(),
            is_admin: user.permissions.contains(&"admin".to_string()),
            banned: user.banned,
        })
    }

    #[tracing::instrument(skip(self), name = "auth.get_full_user", fields(user_id = id), err)]
    pub async fn get_full_user(&self, id: &str) -> CoreResult<User> {
        let user_id = UserId::from_str(id)
            .map_err(|_| CoreError::InvalidInput("Invalid user ID format".to_string()))?;
        self.user_repo
            .find_by_id(user_id)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))
    }

    /// Fetch user's Discord guilds from Discord API using OAuth token.
    /// Returns a list of guild IDs where the user is a member.
    pub async fn fetch_discord_guilds_for_user(
        &self,
        access_token: &str,
    ) -> CoreResult<Vec<GuildInfo>> {
        let response = self
            .client
            .get("https://discord.com/api/users/@me/guilds")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| CoreError::Internal(format!("Failed to fetch guilds from Discord: {}", e)))?;

        if !response.status().is_success() {
            return Err(CoreError::Unauthorized(format!(
                "Discord API error: {}",
                response.status()
            )));
        }

        let guilds: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| CoreError::Internal(format!("Failed to parse Discord response: {}", e)))?;

        let guild_infos: Vec<GuildInfo> = guilds
            .into_iter()
            .filter_map(|g| {
                let id: u64 = g["id"].as_str()?.parse().ok()?;
                let name = g["name"].as_str()?.to_string();
                let icon = g["icon"].as_str().map(|icon| {
                    format!("https://cdn.discordapp.com/icons/{}/{}.png", id, icon)
                });

                // Check if user has MANAGE_GUILD permission (bit 5 = 0x20)
                let permissions_str = g["permissions"].as_str()?;
                let permissions: u64 = permissions_str.parse().ok()?;
                let can_manage = (permissions & 0x20) != 0;

                Some(GuildInfo {
                    id,
                    name,
                    icon_url: icon,
                    can_manage,
                })
            })
            .collect();

        Ok(guild_infos)
    }

    #[tracing::instrument(skip(self), name = "auth.refresh_token", fields(user_id = id), err)]
    pub async fn refresh_token(&self, id: &str) -> CoreResult<AuthResponseDto> {
        let user_id = UserId::from_str(id)
            .map_err(|_| CoreError::InvalidInput("Invalid user ID format".to_string()))?;

        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))?;

        if user.banned {
            return Err(CoreError::Forbidden("User is banned".to_string()));
        }

        // Generate JWT
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::days(7))
            .expect("valid timestamp")
            .timestamp();

        let claims = Claims {
            sub: user.id.0.to_string(),
            exp: expiration as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )?;

        let auth_user = AuthUserDto {
            id: user.id.0.to_string(),
            discord_id: user.discord_user_id.0.clone(),
            username: user.username.0.clone(),
            avatar: user.avatar_url.unwrap_or_default(),
            email: user.email.clone(),
            is_admin: user.permissions.contains(&"admin".to_string()),
            banned: user.banned,
        };

        Ok(AuthResponseDto {
            token,
            user: auth_user,
            redirect_url: None,
        })
    }

    pub async fn list_all_users(&self, page: u32, per_page: u32) -> CoreResult<(Vec<User>, u64)> {
        self.user_repo.list_all(page, per_page).await
    }

    pub async fn get_user_internal(&self, id: UserId) -> CoreResult<Option<User>> {
        self.user_repo.find_by_id(id).await
    }

    pub async fn update_user_permissions(
        &self,
        id: UserId,
        permissions: Vec<String>,
    ) -> CoreResult<User> {
        self.user_repo.update_permissions(id, permissions).await
    }

    pub async fn ban_user(&self, id: UserId) -> CoreResult<User> {
        self.user_repo.set_banned_status(id, true).await
    }

    pub async fn unban_user(&self, id: UserId) -> CoreResult<User> {
        self.user_repo.set_banned_status(id, false).await
    }
}
