use crate::repo::UserRepository;
use crate::{AppConfig, CoreError, CoreResult};
use hq_types::hq::{AuthResponseDto, AuthUserDto, LoginResponseDto, User};
use jsonwebtoken::{EncodingKey, Header, encode};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

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

    pub fn get_login_url(&self) -> LoginResponseDto {
        let redirect_url = format!(
            "https://discord.com/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify%20email",
            self.config.discord_client_id,
            urlencoding::encode(&self.config.discord_redirect_uri)
        );
        LoginResponseDto { redirect_url }
    }

    pub async fn authenticate(&self, code: &str) -> CoreResult<AuthResponseDto> {
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
            .get_or_create_user(discord_id, username, avatar.as_deref(), email.as_deref())
            .await?;

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
            is_admin: false, // For MVP
        };

        Ok(AuthResponseDto {
            token,
            user: auth_user,
        })
    }

    pub async fn get_or_create_user(
        &self,
        discord_id: &str,
        username: &str,
        avatar: Option<&str>,
        email: Option<&str>,
    ) -> CoreResult<User> {
        match self.user_repo.find_by_discord_id(discord_id).await? {
            Some(u) => {
                // Here we might want to update user if info changed, but for now just return
                Ok(u)
            }
            None => {
                let mut new_user =
                    User::new(Uuid::new_v4(), discord_id.to_string(), username.to_string());

                if let Some(a) = avatar {
                    new_user.avatar_url = Some(format!(
                        "https://cdn.discordapp.com/avatars/{}/{}",
                        discord_id, a
                    ));
                }
                new_user.email = email.map(|s| s.to_string());

                self.user_repo.create(&new_user).await
            }
        }
    }

    pub async fn get_user(&self, id: &str) -> CoreResult<AuthUserDto> {
        let uuid = Uuid::parse_str(id)
            .map_err(|_| CoreError::InvalidInput("Invalid user ID format".to_string()))?;
        let user = self
            .user_repo
            .find_by_id(uuid)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))?;

        Ok(AuthUserDto {
            id: user.id.0.to_string(),
            discord_id: user.discord_user_id.0.clone(),
            username: user.username.0.clone(),
            avatar: user.avatar_url.unwrap_or_default(),
            email: user.email.clone(),
            is_admin: false, // For MVP
        })
    }
}
