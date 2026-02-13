use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub redis_url: String,
    pub discord_token: String,
    pub port: u16,
    pub host: String,

    // Telemetry configuration
    #[serde(default = "default_service_name")]
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
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

    pub fn addr(&self) -> std::net::SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("Invalid host/port configuration")
    }
}
