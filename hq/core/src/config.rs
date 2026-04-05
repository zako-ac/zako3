use crate::CoreResult;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
    pub discord_redirect_uri: String,
    pub discord_bot_token: String,
    pub jwt_secret: String,
    pub backend_address: String,
    pub rpc_address: String,
    pub redis_url: String,
    pub rpc_admin_token: String,
}

impl AppConfig {
    pub fn load() -> CoreResult<Self> {
        dotenv().ok();

        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            discord_client_id: env::var("DISCORD_CLIENT_ID")?,
            discord_client_secret: env::var("DISCORD_CLIENT_SECRET")?,
            discord_redirect_uri: env::var("DISCORD_REDIRECT_URI")?,
            discord_bot_token: env::var("DISCORD_BOT_TOKEN")?,
            jwt_secret: env::var("JWT_SECRET")?,
            backend_address: env::var("BACKEND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            rpc_address: env::var("RPC_ADDRESS").unwrap_or_else(|_| "127.0.0.1:50051".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            rpc_admin_token: env::var("RPC_ADMIN_TOKEN")?,
        })
    }
}
