use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(default = "default_ae_port")]
    pub ae_port: u16,
    /// Optional override for advertised address (e.g., "host:port" or "http://host:port").
    /// If set, the heuristic address resolver is skipped.
    pub ae_advertise_addr: Option<String>,

    // --- HQ control plane ---
    /// HQ's RPC endpoint.
    ///
    /// The engine registers here and keeps that registration alive, and HQ uses
    /// the address this engine advertises to dispatch audio commands back. That
    /// makes this the one address an engine needs, whether or not it also uses
    /// the v4 audio plane.
    #[serde(default = "default_hq_rpc_url")]
    pub hq_rpc_url: String,
    pub hq_rpc_admin_token: Option<String>,
    #[serde(default = "default_hq_request_timeout_ms")]
    pub hq_request_timeout_ms: u64,
    /// Turns on the v4 audio plane (protofish4 ingest + sink advertisement).
    /// Off means this engine serves audio through taphub only.
    #[serde(default)]
    pub audio_plane_v4: bool,

    // --- Discord identity ---
    /// The Discord bot this engine logs in as.
    ///
    /// `DISCORD_TOKEN` wins when set — the single-engine / local-dev case.
    /// Otherwise `DISCORD_TOKENS` is a comma-separated pool indexed by this
    /// pod's StatefulSet ordinal (see [`AppConfig::resolve_discord_token`]).
    pub discord_token: Option<String>,
    pub discord_tokens: Option<String>,

    /// Redis used to persist active voice sessions so a restarted AE can rejoin them.
    #[serde(default = "default_redis_url")]
    pub redis_url: String,
    #[serde(default = "default_taphub_url")]
    pub taphub_url: String,
    #[serde(default = "default_taphub_sni")]
    pub taphub_sni: String,
    #[serde(default = "default_taphub_transport_cert_file")]
    pub taphub_transport_cert_file: String,
    #[serde(default = "default_taphub_request_timeout_ms")]
    pub taphub_request_timeout_ms: u64,

    /// Where this engine receives audio over UDP. Only used when the v4 audio
    /// plane is enabled.
    #[serde(default = "default_udp_bind_addr")]
    pub udp_bind_addr: String,
    /// Reachable from the internet, once this engine has its own public IP.
    /// Unset while the shared proxy owns the only one.
    pub udp_public_addr: Option<String>,
    /// Overrides the id this engine advertises itself under. Defaults to the
    /// pod name, which is stable across restarts in a StatefulSet.
    pub sink_id: Option<String>,
    #[serde(default = "default_cache_rpc_url")]
    pub cache_rpc_url: String,
    pub cache_rpc_admin_token: Option<String>,

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

/// Kept under the engine's per-attempt budget so a slow HQ surfaces as an HQ
/// timeout rather than being swallowed by the outer command backstop.
fn default_hq_request_timeout_ms() -> u64 {
    6000
}

fn default_udp_bind_addr() -> String {
    "0.0.0.0:5000".to_string()
}

fn default_cache_rpc_url() -> String {
    "http://localhost:4100".to_string()
}

/// Where HQ's RPC server listens by default.
fn default_hq_rpc_url() -> String {
    "http://127.0.0.1:50052".to_string()
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
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

/// Per-attempt taphub request timeout. Chosen to satisfy the timeout hierarchy so a
/// command that makes back-to-back taphub calls (a normal `play`: `request_audio_meta`
/// then `request_audio`) still finishes inside the 25s command backstop:
///   N_seq(2) × MAX_ATTEMPTS(2) × per_attempt(6s) = 24s ≤ backstop(25s) < HQ dispatch(30s).
/// If legitimately-slow calls need more than 6s, raise this AND the backstop
/// (`server.rs`) and HQ's dispatch timeout
/// (`services/hq/core/src/service/ae_registry.rs`) together to keep the inequality
/// holding.
fn default_taphub_request_timeout_ms() -> u64 {
    6_000
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

impl AppConfig {
    /// How this engine identifies itself to HQ and in the sink registry.
    ///
    /// Must be stable across a restart: HQ keys its registry on it (so a
    /// restart replaces the old entry instead of adding a second one), and a
    /// new id every restart would leave stale advertisements behind until
    /// their leases lapse.
    pub fn sink_id(&self) -> String {
        self.sink_id
            .clone()
            .or_else(|| std::env::var("POD_NAME").ok())
            .or_else(|| std::env::var("HOSTNAME").ok())
            .unwrap_or_else(|| "audio-engine".to_string())
    }

    /// This pod's StatefulSet ordinal, parsed off the trailing `-N` of its own
    /// name (`zako3-audio-engine-3` → `3`).
    ///
    /// `None` outside a StatefulSet (local dev, where `HOSTNAME` has no
    /// ordinal), which the token pool treats as ordinal 0.
    pub fn pod_ordinal(&self) -> Option<usize> {
        let name = std::env::var("POD_NAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .ok()?;
        name.rsplit_once('-')?.1.parse::<usize>().ok()
    }

    /// The Discord token this engine must log in as.
    ///
    /// The engine is a StatefulSet, so one token per pod ordinal has to be
    /// expressible. Rather than deriving a value or running an init container,
    /// the whole pool is handed to every pod and each pod picks its own index:
    /// pod `-N` takes entry N of the comma-separated list, which is exactly the
    /// mapping the previous token broker used. Keeping it identical is what
    /// preserves the Redis session namespace, since that is derived from the
    /// token — a different token here would silently orphan every session this
    /// engine was supposed to rejoin.
    ///
    /// Refuses to fall back to another pod's token when the ordinal is out of
    /// range: two pods logging in as the same bot makes Discord drop one of
    /// them, so failing loudly is the only safe answer.
    pub fn resolve_discord_token(&self) -> Option<String> {
        if let Some(token) = self
            .discord_token
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            return Some(token.to_string());
        }

        let pool: Vec<&str> = self
            .discord_tokens
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect();

        if pool.is_empty() {
            return None;
        }

        let ordinal = self.pod_ordinal().unwrap_or(0);
        match pool.get(ordinal) {
            Some(token) => Some((*token).to_string()),
            None => {
                tracing::error!(
                    ordinal,
                    pool = pool.len(),
                    "DISCORD_TOKENS has fewer tokens than this pod's ordinal; \
                     refusing to log in as another pod's bot"
                );
                None
            }
        }
    }

    /// The address the proxy forwards to.
    ///
    /// Derived from the bind address plus this pod's own IP, since binding
    /// `0.0.0.0` says nothing about how to reach it.
    pub fn udp_internal_addr(&self) -> String {
        let port = self
            .udp_bind_addr
            .rsplit_once(':')
            .map(|(_, p)| p.to_string())
            .unwrap_or_else(|| "5000".to_string());
        match std::env::var("POD_IP") {
            Ok(ip) if !ip.is_empty() => format!("{ip}:{port}"),
            _ => self.udp_bind_addr.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_token_wins_over_the_pool() {
        let config = AppConfig {
            discord_token: Some("solo".to_string()),
            discord_tokens: Some("a,b,c".to_string()),
            ..solo_config()
        };
        assert_eq!(config.resolve_discord_token(), Some("solo".to_string()));
    }

    #[test]
    fn pool_is_indexed_by_pod_ordinal() {
        // The ordinal is read from the environment, so this exercises the
        // documented `HOSTNAME`-shaped fallback rather than a real StatefulSet.
        // Held under the shared lock: the address-heuristic test sets the same
        // variable, in the same process, on another thread.
        let _env = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var("POD_NAME");
            std::env::set_var("HOSTNAME", "zako3-audio-engine-1");
        }
        let config = AppConfig {
            discord_token: None,
            discord_tokens: Some("tok0, tok1 ,tok2".to_string()),
            ..solo_config()
        };
        assert_eq!(config.pod_ordinal(), Some(1));
        assert_eq!(config.resolve_discord_token(), Some("tok1".to_string()));

        // An ordinal past the end of the pool must refuse rather than fall back
        // to another pod's bot: two pods on one token makes Discord drop one.
        unsafe {
            std::env::set_var("HOSTNAME", "zako3-audio-engine-5");
        }
        let out_of_range = AppConfig {
            discord_token: None,
            discord_tokens: Some("tok0,tok1".to_string()),
            ..solo_config()
        };
        assert_eq!(out_of_range.resolve_discord_token(), None);
    }

    fn solo_config() -> AppConfig {
        AppConfig {
            ae_port: 8090,
            ae_advertise_addr: None,
            hq_rpc_url: default_hq_rpc_url(),
            hq_rpc_admin_token: None,
            hq_request_timeout_ms: default_hq_request_timeout_ms(),
            audio_plane_v4: false,
            discord_token: None,
            discord_tokens: None,
            redis_url: default_redis_url(),
            taphub_url: default_taphub_url(),
            taphub_sni: default_taphub_sni(),
            taphub_transport_cert_file: default_taphub_transport_cert_file(),
            taphub_request_timeout_ms: default_taphub_request_timeout_ms(),
            udp_bind_addr: default_udp_bind_addr(),
            udp_public_addr: None,
            sink_id: None,
            cache_rpc_url: default_cache_rpc_url(),
            cache_rpc_admin_token: None,
            service_name: default_service_name(),
            otlp_endpoint: None,
            metrics_port: default_metrics_port(),
        }
    }
}
