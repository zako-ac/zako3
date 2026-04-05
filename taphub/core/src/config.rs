use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub transport_bind_addr: String,
    pub zakofish_bind_addr: String,
    pub hq_rpc_url: String,
    pub hq_rpc_admin_token: String,
    pub cert_file: String,
    pub key_file: String,
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
        })
    }
}
