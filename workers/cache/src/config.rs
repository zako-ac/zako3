use std::{env, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub cache_dir: PathBuf,
    pub admin_token: Option<String>,
    pub redis_url: Option<String>,
    pub otlp_endpoint: Option<String>,
    pub metrics_port: Option<u16>,
    /// Sidecar reads in flight while the index warms up at startup.
    pub warmup_concurrency: usize,
    /// How long a preload session may sit idle before it is collected.
    ///
    /// Generous, because a legitimately slow producer must not be cut off — but
    /// finite, because a producer that vanishes gives no other signal, and a
    /// session left behind keeps `GET /stream` pointed at a dead partial file
    /// for that cache key. Zero disables the reaper.
    pub preload_session_ttl: Duration,
    pub gc: GcConfig,
    /// UDP ingest, when it is configured. `None` leaves the worker HTTP-only,
    /// which is what every pre-v4 deployment stays as.
    pub udp: Option<UdpConfig>,
}

/// The cache worker as a protofish4 sink.
#[derive(Debug, Clone)]
pub struct UdpConfig {
    pub bind_addr: String,
    /// How this worker is named in the sink registry. HQ resolves the same id
    /// to fill `deliver_to`, so the two must agree.
    pub sink_id: String,
    /// Where the proxy forwards to. Defaults to the bind address, which is
    /// wrong under a wildcard bind — hence the override.
    pub internal_addr: String,
    /// Set only once this worker has a public IP of its own, at which point
    /// taps are pointed straight here and the proxy drops out of the path.
    pub public_addr: Option<String>,
    /// Concurrent transfers. Each holds a reorder window, so this is the
    /// memory bound.
    pub max_sessions: usize,
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

        let warmup_concurrency = env::var("ZK_CACHE_WARMUP_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(zako3_preload_cache::DEFAULT_WARMUP_CONCURRENCY);

        let preload_session_ttl = Duration::from_secs(
            env::var("ZK_CACHE_PRELOAD_SESSION_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(300),
        );

        // Ingest is off unless a bind address is given: the port has to be
        // opened in the deployment anyway, so making it explicit keeps a
        // half-configured worker from advertising a sink nothing can reach.
        let udp = env::var("ZK_CACHE_UDP_BIND_ADDR")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|bind_addr| {
                let internal_addr = env::var("ZK_CACHE_UDP_INTERNAL_ADDR")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| bind_addr.clone());
                UdpConfig {
                    bind_addr,
                    sink_id: env::var("ZK_CACHE_SINK_ID")
                        .ok()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "cache".to_string()),
                    internal_addr,
                    public_addr: env::var("ZK_CACHE_UDP_PUBLIC_ADDR")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    max_sessions: env::var("ZK_CACHE_UDP_MAX_SESSIONS")
                        .ok()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|v| *v > 0)
                        .unwrap_or(64),
                }
            });

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
            warmup_concurrency,
            preload_session_ttl,
            udp,
            gc: GcConfig {
                interval: Duration::from_secs(interval_secs),
                max_bytes,
                batch_size,
            },
        })
    }
}
