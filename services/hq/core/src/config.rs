use crate::CoreResult;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub timescale_database_url: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
    pub discord_redirect_uri: String,
    pub discord_bot_token: String,
    pub jwt_secret: String,
    pub backend_address: String,
    pub rpc_address: String,
    pub redis_url: String,
    pub rpc_admin_token: String,
    pub zako_website_url: String,
    pub traffic_light_url: String,
    pub mapper_wasm_dir: PathBuf,
    pub mapper_db_path: PathBuf,
    pub otlp_endpoint: Option<String>,
    pub metrics_port: Option<u16>,
}

impl AppConfig {
    pub fn load() -> CoreResult<Self> {
        dotenv().ok();

        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            timescale_database_url: env::var("TIMESCALE_DATABASE_URL")?,
            discord_client_id: env::var("DISCORD_CLIENT_ID")?,
            discord_client_secret: env::var("DISCORD_CLIENT_SECRET")?,
            discord_redirect_uri: env::var("DISCORD_REDIRECT_URI")?,
            discord_bot_token: env::var("DISCORD_BOT_TOKEN")?,
            jwt_secret: env::var("JWT_SECRET")?,
            backend_address: env::var("BACKEND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            rpc_address: env::var("RPC_ADDRESS")?,
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            rpc_admin_token: env::var("RPC_ADMIN_TOKEN")?,
            zako_website_url: env::var("ZAKO_WEBSITE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            traffic_light_url: env::var("TRAFFIC_LIGHT_URL")
                .unwrap_or_else(|_| "127.0.0.1:7070".to_string()),
            mapper_wasm_dir: env::var("MAPPER_WASM_DIR").map(PathBuf::from)?,
            mapper_db_path: env::var("MAPPER_DB_PATH").map(PathBuf::from)?,
            otlp_endpoint: env::var("OTLP_ENDPOINT").ok(),
            metrics_port: env::var("METRICS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .or(Some(9091)),
        })
    }
}
