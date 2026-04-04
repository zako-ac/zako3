use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: String,
    pub zakofish_bind_addr: String,
    pub cert_file: String,
    pub key_file: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            bind_addr: env::var("ZK_TH_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:4000".to_string()),
            zakofish_bind_addr: env::var("ZK_TH_ZAKOFISH_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:4001".to_string()),
            cert_file: env::var("ZK_TH_CERT_FILE").unwrap_or_else(|_| "cert.pem".to_string()),
            key_file: env::var("ZK_TH_KEY_FILE").unwrap_or_else(|_| "key.pem".to_string()),
        })
    }
}
