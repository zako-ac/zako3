use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub nats_url: String,
    pub http_addr: String,
    pub otlp_endpoint: Option<String>,
    pub database_url: String,
}

impl AppConfig {
    pub fn load() -> Self {
        dotenvy::dotenv().ok();

        match envy::from_env::<AppConfig>() {
            Ok(config) => config,
            Err(e) => {
                tracing::error!("Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        }
    }
}
