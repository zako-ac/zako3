use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:4000".to_string()),
        })
    }
}
