use std::env;
use std::time::Duration;

/// Explicit `env::var` with defaults rather than `envy`, because the settings
/// here are prefixed and `envy` cannot express a prefix. Shared infrastructure
/// keys (`REDIS_URL`, `OTLP_ENDPOINT`, `METRICS_PORT`) stay unprefixed, as in
/// every other worker.
#[derive(Debug, Clone)]
pub struct Config {
    /// The public UDP address taps send to.
    pub bind_addr: String,
    /// Where `/healthz` and `/metrics` are served. UDP cannot be probed, so
    /// this is what the kubelet checks.
    pub http_addr: String,
    pub redis_url: String,
    pub otlp_endpoint: Option<String>,
    /// Hard cap on concurrent request ids.
    pub max_routes: usize,
    /// Drop a route after this long with no datagram in either direction.
    ///
    /// This is what actually bounds the table: HQ writes routes with a one-hour
    /// TTL and does not always delete them on completion, so eviction here is
    /// load-bearing rather than a safety net.
    pub route_idle: Duration,
    /// How long an unroutable request id is remembered as unroutable, so a
    /// spray of guessed ids does not become a Redis load test.
    pub negative_ttl: Duration,
    /// Datagrams held across every in-flight route lookup, together. The
    /// opening packets of a transfer arrive before its route is known, and a
    /// sender that opens with a burst can lose all of them at once.
    pub max_pending: usize,
    /// Cap on remembered-unroutable ids, held separately from live routes.
    ///
    /// Same map would mean an attacker choosing 128-bit ids could evict every
    /// live transfer, which is a far worse outcome than losing the negative
    /// cache.
    pub max_negative: usize,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        Ok(Self {
            bind_addr: var("ZK_AEP_BIND_ADDR").unwrap_or_else(|| "0.0.0.0:5000".to_string()),
            http_addr: format!(
                "0.0.0.0:{}",
                var("METRICS_PORT").unwrap_or_else(|| "9095".to_string())
            ),
            redis_url: var("REDIS_URL")
                .ok_or_else(|| anyhow::anyhow!("REDIS_URL is required"))?,
            otlp_endpoint: var("OTLP_ENDPOINT"),
            max_routes: parse("ZK_AEP_MAX_ROUTES", 4096),
            route_idle: Duration::from_secs(parse("ZK_AEP_ROUTE_IDLE_SECONDS", 300)),
            negative_ttl: Duration::from_secs(parse("ZK_AEP_NEGATIVE_TTL_SECONDS", 5)),
            max_pending: parse("ZK_AEP_MAX_PENDING", 512),
            max_negative: parse("ZK_AEP_MAX_NEGATIVE", 4096),
        })
    }
}

fn var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

fn parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    var(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}
