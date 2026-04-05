use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub redis_url: String,
    pub discord_token: String,
    #[serde(default = "default_rabbitmq_url")]
    pub rabbitmq_url: String,
    #[serde(default = "default_ae_max_retries")]
    pub ae_max_retries: u32,

    // Telemetry configuration
    #[serde(default = "default_service_name")]
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_rabbitmq_url() -> String {
    "amqp://127.0.0.1:5672/%2f".to_string()
}

fn default_ae_max_retries() -> u32 {
    10
}

fn default_service_name() -> String {
    "audio-engine".to_string()
}

fn default_metrics_port() -> u16 {
    9090
}

impl AppConfig {
    pub fn load() -> Self {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        // Enforce required variables via envy
        match envy::from_env::<AppConfig>() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        }
    }
}
