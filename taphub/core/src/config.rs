use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub transport_bind_addr: String,
    pub zakofish_bind_addr: String,
    pub hq_rpc_url: String,
    pub hq_rpc_admin_token: String,
    pub cert_file: String,
    pub key_file: String,
    pub redis_url: String,
    pub cache_dir: PathBuf,
    pub request_timeout_ms: u64,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            transport_bind_addr: env::var("ZK_TH_TRANSPORT_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:4000".to_string()),
            zakofish_bind_addr: env::var("ZK_TH_ZAKOFISH_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:4001".to_string()),
            hq_rpc_url: env::var("ZK_TH_HQ_RPC_URL")
                .unwrap_or_else(|_| "http://localhost:4002".to_string()),
            hq_rpc_admin_token: env::var("ZK_TH_HQ_RPC_ADMIN_TOKEN")?,
            cert_file: env::var("ZK_TH_CERT_FILE").unwrap_or_else(|_| "cert.pem".to_string()),
            key_file: env::var("ZK_TH_KEY_FILE").unwrap_or_else(|_| "key.pem".to_string()),
            redis_url: env::var("ZK_TH_REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            cache_dir: PathBuf::from(
                env::var("ZK_TH_CACHE_DIR").unwrap_or_else(|_| "/tmp/zako3-cache".to_string()),
            ),
            request_timeout_ms: env::var("ZK_TH_REQUEST_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10_000),
        })
    }
}
