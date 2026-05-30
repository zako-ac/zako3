use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub transport_bind_addr: String,
    pub zakofish_bind_addr: String,
    /// Optional pf3 bind address. When set, the Hub also accepts pf3 (protofish3)
    /// tap connections on this port (using the same cert/key as the pf2 port).
    pub zakofish_bind_addr_pf3: Option<String>,
    pub hq_rpc_url: String,
    pub hq_rpc_admin_token: String,
    pub zakofish_cert_file: String,
    pub zakofish_key_file: String,
    pub transport_cert_file: String,
    pub transport_key_file: String,
    pub redis_url: String,
    pub cache_rpc_url: String,
    pub cache_rpc_admin_token: Option<String>,
    pub request_timeout_ms: u64,
    /// TTL (seconds) for published tap connection-state leases in Redis. The hub
    /// refreshes them on a heartbeat at roughly `ttl / 3`; if the process dies the
    /// keys expire after this window, so stale "online" state cannot linger.
    pub connection_lease_ttl_secs: u64,
    pub otlp_endpoint: Option<String>,
    pub metrics_port: Option<u16>,
    pub bypass_hq: bool,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        dotenvy::dotenv().ok();

        let bypass_hq = env::var("ZK_TH_BYPASS_HQ")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "True"))
            .unwrap_or(false);

        let hq_rpc_admin_token = if bypass_hq {
            env::var("ZK_TH_HQ_RPC_ADMIN_TOKEN").unwrap_or_default()
        } else {
            env::var("ZK_TH_HQ_RPC_ADMIN_TOKEN")?
        };

        Ok(Self {
            transport_bind_addr: env::var("ZK_TH_TRANSPORT_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:4000".to_string()),
            zakofish_bind_addr: env::var("ZK_TH_ZAKOFISH_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:4001".to_string()),
            zakofish_bind_addr_pf3: env::var("ZK_TH_ZAKOFISH_BIND_ADDR_PF3")
                .ok()
                .filter(|s| !s.is_empty()),
            hq_rpc_url: env::var("ZK_TH_HQ_RPC_URL")
                .unwrap_or_else(|_| "http://localhost:4002".to_string()),
            hq_rpc_admin_token,
            zakofish_cert_file: env::var("ZK_TH_ZAKOFISH_CERT_FILE")
                .unwrap_or_else(|_| "cert.pem".to_string()),
            zakofish_key_file: env::var("ZK_TH_ZAKOFISH_KEY_FILE")
                .unwrap_or_else(|_| "key.pem".to_string()),
            transport_cert_file: env::var("ZK_TH_TAPHUB_TRANSPORT_CERT_FILE")
                .unwrap_or_else(|_| "cert.pem".to_string()),
            transport_key_file: env::var("ZK_TH_TAPHUB_TRANSPORT_KEY_FILE")
                .unwrap_or_else(|_| "key.pem".to_string()),
            redis_url: env::var("ZK_TH_REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            cache_rpc_url: env::var("ZK_TH_CACHE_RPC_URL")
                .unwrap_or_else(|_| "http://localhost:4100".to_string()),
            cache_rpc_admin_token: env::var("ZK_TH_CACHE_RPC_ADMIN_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            request_timeout_ms: env::var("ZK_TH_REQUEST_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10_000),
            connection_lease_ttl_secs: env::var("ZK_TH_CONNECTION_LEASE_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            otlp_endpoint: env::var("ZK_TH_OTLP_ENDPOINT").ok(),
            metrics_port: env::var("ZK_TH_METRICS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .or(Some(9092)),
            bypass_hq,
        })
    }
}
