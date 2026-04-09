use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub redis_url: String,
    pub discord_token: String,
    #[serde(default = "default_nats_url")]
    pub nats_url: String,
    #[serde(default = "default_taphub_url")]
    pub taphub_url: String,
    #[serde(default = "default_taphub_sni")]
    pub taphub_sni: String,

    // Telemetry configuration
    #[serde(default = "default_service_name")]
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_nats_url() -> String {
    "nats://127.0.0.1:4222".to_string()
}

fn default_taphub_url() -> String {
    "127.0.0.1:4000".to_string()
}

fn default_taphub_sni() -> String {
    "localhost".to_string()
}

fn default_service_name() -> String {
    "audio-engine".to_string()
}

fn default_metrics_port() -> u16 {
    9090
}

impl AppConfig {
    pub fn load() -> Self {
        dotenvy::dotenv().ok();

        match envy::from_env::<AppConfig>() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        }
    }
}
