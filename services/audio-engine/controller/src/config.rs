use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(default = "default_ae_port")]
    pub ae_port: u16,
    #[serde(default = "default_tl_rpc_url")]
    pub tl_rpc_url: String,
    #[serde(default = "default_taphub_url")]
    pub taphub_url: String,
    #[serde(default = "default_taphub_sni")]
    pub taphub_sni: String,
    #[serde(default = "default_taphub_transport_cert_file")]
    pub taphub_transport_cert_file: String,

    // Telemetry configuration
    #[serde(default = "default_service_name")]
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_ae_port() -> u16 {
    8090
}

fn default_tl_rpc_url() -> String {
    "http://127.0.0.1:7070".to_string()
}

fn default_taphub_url() -> String {
    "127.0.0.1:4000".to_string()
}

fn default_taphub_sni() -> String {
    "localhost".to_string()
}

fn default_taphub_transport_cert_file() -> String {
    "cert.pem".to_string()
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
