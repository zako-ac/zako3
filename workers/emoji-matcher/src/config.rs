use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub nats_url: String,
    pub http_addr: String,
    pub otlp_endpoint: Option<String>,
    pub database_url: String,
    pub redis_url: String,
    #[serde(default = "default_concurrency")]
    pub worker_concurrency: usize,
    #[serde(default = "default_hamming_threshold")]
    pub match_hamming_threshold: u32,
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,
}

fn default_concurrency() -> usize {
    8
}

fn default_hamming_threshold() -> u32 {
    4
}

fn default_queue_capacity() -> usize {
    1024
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
