use std::{env, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub cache_dir: PathBuf,
    pub admin_token: Option<String>,
    pub redis_url: Option<String>,
    pub otlp_endpoint: Option<String>,
    pub metrics_port: Option<u16>,
    pub gc: GcConfig,
}

#[derive(Debug, Clone)]
pub struct GcConfig {
    pub interval: Duration,
    pub max_bytes: Option<u64>,
    pub batch_size: usize,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let cache_dir = PathBuf::from(
            env::var("ZK_CACHE_DIR").unwrap_or_else(|_| "/cache".to_string()),
        );
        let bind_addr = env::var("ZK_CACHE_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:4100".to_string());

        let admin_token = env::var("ZK_CACHE_ADMIN_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());

        // Backwards-compatible env name for the redis URL (matches existing
        // cache-gc usage so dashboards / helm don't have to change in lockstep).
        let redis_url = env::var("REDIS_URL")
            .ok()
            .or_else(|| env::var("ZK_CACHE_REDIS_URL").ok())
            .filter(|s| !s.is_empty());

        let otlp_endpoint = env::var("OTLP_ENDPOINT")
            .ok()
            .filter(|s| !s.is_empty());

        let metrics_port = env::var("ZK_CACHE_METRICS_PORT")
            .ok()
            .and_then(|v| v.parse().ok());

        let interval_secs = env::var("ZK_CACHE_GC_INTERVAL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30 * 60);
        let max_bytes = env::var("ZK_CACHE_MAX_BYTES")
            .ok()
            .and_then(|v| v.parse().ok());
        let batch_size = env::var("ZK_CACHE_GC_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        Ok(Self {
            bind_addr,
            cache_dir,
            admin_token,
            redis_url,
            otlp_endpoint,
            metrics_port,
            gc: GcConfig {
                interval: Duration::from_secs(interval_secs),
                max_bytes,
                batch_size,
            },
        })
    }
}
