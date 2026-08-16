use std::net::SocketAddr;
use std::sync::Arc;

use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::http_client::HttpClient;
use jsonrpsee::server::Server;
use serde::Deserialize;
use tl_protocol::TrafficLightRpcClient;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, TrafficLightRpcServer};
use tokio::sync::RwLock;
use tracing::info;
use zako3_telemetry::TelemetryConfig;
use zako3_tl_core::{DiscordToken, TlService, Worker, WorkerId, WorkerPermissions, ZakoState};
use zako3_tl_infra::AeRegistry;
use zako3_types::hq::DiscordUserId;
use zako3_types::{GuildId, SessionState};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AppConfig {
    // Telemetry
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    // RPC listener (callers connect here)
    #[serde(default = "default_rpc_addr")]
    pub rpc_addr: SocketAddr,

    // Comma-separated Discord bot tokens, e.g. "tokenA,tokenB,tokenC"
    pub discord_tokens: String,
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_rpc_addr() -> SocketAddr {
    "0.0.0.0:7070".parse().unwrap()
}

/// Derives a Discord bot's client (== user) id from its token. A bot token's
/// first `.`-separated segment is the base64url-encoded ASCII user id.
fn client_id_from_token(token: &str) -> Option<String> {
    use base64::Engine as _;
    let first = token.split('.').next()?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(first)
        .ok()?;
    let s = String::from_utf8(bytes).ok()?;
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then_some(s)
}

impl AppConfig {
    fn load() -> Self {
        dotenvy::dotenv().ok();
        match envy::from_env::<AppConfig>() {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Failed to load config: {e}");
                std::process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// jsonrpsee service impl
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TrafficLightServiceImpl {
    tl: Arc<TlService>,
    ae_registry: Arc<AeRegistry>,
}

#[async_trait]
impl TrafficLightRpcServer for TrafficLightServiceImpl {
    async fn execute(
        &self,
        request: AudioEngineCommandRequest,
    ) -> RpcResult<AudioEngineCommandResponse> {
        Ok(self.tl.execute(request).await)
    }

    async fn get_sessions_in_guild(&self, guild_id: GuildId) -> RpcResult<Vec<SessionState>> {
        Ok(self.tl.get_sessions_in_guild(guild_id).await)
    }

    async fn list_bot_ids(&self) -> RpcResult<Vec<String>> {
        Ok(self.tl.list_bot_ids().await)
    }

    async fn report_guilds(&self, token: String, guilds: Vec<GuildId>) -> RpcResult<()> {
        Ok(self.tl.report_guilds(token, guilds).await)
    }

    async fn register_ae(&self, listen_addr: String) -> RpcResult<String> {
        match self.ae_registry.register(listen_addr).await {
            Ok(token) => Ok(token),
            Err(zako3_tl_infra::RegistrationError::InvalidListenAddress(msg)) => {
                tracing::warn!("Invalid listen address: {}", msg);
                Err(jsonrpsee::types::error::ErrorObject::owned(
                    jsonrpsee::types::error::INVALID_PARAMS_CODE,
                    msg,
                    None::<()>,
                )
                .into())
            }
            Err(e) => {
                tracing::error!("Failed to register AE: {:?}", e);
                Err(jsonrpsee::types::error::ErrorCode::InternalError.into())
            }
        }
    }

    async fn heartbeat_ae(&self, token: String, listen_addr: String) -> RpcResult<()> {
        match self.ae_registry.heartbeat(token, listen_addr).await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::warn!("heartbeat_ae failed: {e}");
                Err(jsonrpsee::types::error::ErrorCode::InternalError.into())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load();

    // Parse token pool from comma-separated env var
    let tokens: Vec<DiscordToken> = config
        .discord_tokens
        .split(',')
        .map(|s| DiscordToken(s.trim().to_string()))
        .filter(|t| !t.0.is_empty())
        .collect();

    if tokens.is_empty() {
        eprintln!("DISCORD_TOKENS must contain at least one token");
        std::process::exit(1);
    }

    info!("Token pool: {} token(s)", tokens.len());

    // Init telemetry (provides /health and /metrics on metrics_port)
    let telemetry = zako3_telemetry::init(TelemetryConfig {
        service_name: "traffic-light".to_string(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: Some(config.metrics_port),
    })
    .await?;

    // Pre-populate workers from token list (one worker per token)
    let workers = tokens
        .iter()
        .enumerate()
        .map(|(i, token)| {
            let worker_id = WorkerId(i as u16);
            let bot_client_id = client_id_from_token(&token.0).unwrap_or_else(|| {
                tracing::warn!("Could not derive bot client id from token for {worker_id:?}");
                String::new()
            });
            let worker = Worker {
                worker_id,
                bot_client_id: DiscordUserId(bot_client_id),
                discord_token: token.clone(),
                connected_ae_ids: vec![],
                permissions: WorkerPermissions::new(),
            };
            (worker_id, worker)
        })
        .collect();

    let initial_state = ZakoState {
        workers,
        sessions: Default::default(),
    };

    // Build AE registry (now HTTP-based, AEs register themselves)
    let state = Arc::new(RwLock::new(initial_state));
    let ae_registry = Arc::new(AeRegistry::new(state.clone(), tokens).await?);
    info!("AE registry initialized; AEs will register via register_ae RPC");

    // Build TlService backed by the AE registry — shares the same state Arc so
    // accept_loop writes (connected_ae_ids) are immediately visible to the router.
    let tl_service = Arc::new(TlService::new(ae_registry.state(), ae_registry.clone()));

    // Run reconcile on boot to clean up any dangling sessions from previous crashes
    tl_service.reconcile().await;

    // Spawn session sync task — fetches current session state from all AEs every 60 seconds
    let tl_for_sync = tl_service.clone();
    let sync_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            tl_for_sync.sync_sessions().await;
        }
    });

    // Spawn reconcile task — detects dangling sessions and duplicate bots every 60 seconds
    let tl_for_reconcile = tl_service.clone();
    let reconcile_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            tl_for_reconcile.reconcile().await;
            tl_for_reconcile.evict_duplicates().await;
        }
    });

    let svc = TrafficLightServiceImpl {
        tl: tl_service,
        ae_registry,
    };

    // Start JSON-RPC HTTP listener
    let server = Server::builder().build(config.rpc_addr).await?;
    info!("RPC server listening on {}", config.rpc_addr);

    // Self-health-check: periodically round-trip an RPC request against our own
    // address so a dead serving layer (accept loop wedged/stopped) surfaces as
    // /health=503 and, after enough consecutive failures, exits the process so
    // k8s restarts us — instead of lingering silently alive with a dead listener.
    let health_telemetry = telemetry.clone();
    let health_rpc_addr = config.rpc_addr;
    let health_handle = tokio::spawn(async move {
        let url = format!("http://{}", health_rpc_addr);
        let mut consecutive_failures: u32 = 0;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            // Build a fresh client each round to avoid a stale pooled connection.
            let client = match HttpClient::builder()
                .request_timeout(std::time::Duration::from_secs(10))
                .build(&url)
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("self-health: failed to build RPC client: {e}");
                    consecutive_failures += 1;
                    health_telemetry.unhealthy();
                    if consecutive_failures >= 3 {
                        tracing::error!(
                            "self-health: RPC client unreachable after {consecutive_failures} attempts; exiting for k8s restart"
                        );
                        std::process::exit(1);
                    }
                    continue;
                }
            };
            match client.list_bot_ids().await {
                Ok(_) => {
                    consecutive_failures = 0;
                    health_telemetry.healthy();
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        "self-health: RPC round-trip failed ({consecutive_failures} consecutive): {e}"
                    );
                    health_telemetry.unhealthy();
                    if consecutive_failures >= 3 {
                        tracing::error!(
                            "self-health: RPC serving layer unreachable after {consecutive_failures} consecutive failures; exiting for k8s restart"
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
    });

    telemetry.healthy();
    info!("Traffic Light is ready");

    let handle = server.start(svc.into_rpc());

    // We never call stop(), so if the server handle stops — or any background
    // task exits — the serving layer has died. Exit(1) so k8s restarts the pod
    // instead of leaving a dead-but-alive process (see traffic-light outage RCA).
    tokio::select! {
        _ = handle.stopped() => {
            tracing::error!("RPC server stopped unexpectedly; exiting for k8s restart");
            std::process::exit(1);
        }
        res = sync_handle => {
            tracing::error!("session sync task ended unexpectedly: {res:?}; exiting for k8s restart");
            std::process::exit(1);
        }
        res = reconcile_handle => {
            tracing::error!("reconcile task ended unexpectedly: {res:?}; exiting for k8s restart");
            std::process::exit(1);
        }
        _ = health_handle => {
            tracing::error!("self-health task ended unexpectedly; exiting for k8s restart");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn derives_client_id_from_token() {
        let first = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("123456789");
        let token = format!("{first}.abcdef.ghijkl");
        assert_eq!(client_id_from_token(&token), Some("123456789".to_string()));
    }

    #[test]
    fn rejects_garbage_token() {
        assert_eq!(client_id_from_token("not-base64!.x.y"), None);
        assert_eq!(client_id_from_token(""), None);
    }
}
