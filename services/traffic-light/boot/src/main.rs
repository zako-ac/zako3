use std::net::SocketAddr;
use std::sync::Arc;

use serde::Deserialize;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, TrafficLightRpcServer};
use zako3_types::{GuildId, SessionState};
use tokio::sync::RwLock;
use tracing::info;
use zako3_tl_core::{
    DiscordToken, TlService, Worker, WorkerPermissions, WorkerId, ZakoState,
};
use zako3_tl_infra::AeRegistry;
use zako3_telemetry::TelemetryConfig;
use zako3_types::hq::DiscordUserId;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::server::Server;

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
    async fn execute(&self, request: AudioEngineCommandRequest) -> RpcResult<AudioEngineCommandResponse> {
        Ok(self.tl.execute(request).await)
    }

    async fn get_sessions_in_guild(&self, guild_id: GuildId) -> RpcResult<Vec<SessionState>> {
        Ok(self.tl.get_sessions_in_guild(guild_id).await)
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
            let worker = Worker {
                worker_id,
                bot_client_id: DiscordUserId(String::new()),
                discord_token: token.clone(),
                connected_ae_ids: vec![],
                permissions: WorkerPermissions::new(),
                ae_cursor: 0,
            };
            (worker_id, worker)
        })
        .collect();

    let initial_state = ZakoState {
        workers,
        sessions: Default::default(),
        worker_cursor: 0,
    };

    // Build AE registry (now HTTP-based, AEs register themselves)
    let state = Arc::new(RwLock::new(initial_state));
    let ae_registry = Arc::new(
        AeRegistry::new(state.clone(), tokens).await?,
    );
    info!("AE registry initialized; AEs will register via register_ae RPC");

    // Build TlService backed by the AE registry — shares the same state Arc so
    // accept_loop writes (connected_ae_ids) are immediately visible to the router.
    let tl_service = Arc::new(TlService::new(ae_registry.state(), ae_registry.clone()));

    // Run reconcile on boot to clean up any dangling sessions from previous crashes
    tl_service.reconcile().await;

    // Spawn session sync task — fetches current session state from all AEs every 60 seconds
    let tl_for_sync = tl_service.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            tl_for_sync.sync_sessions().await;
        }
    });

    // Spawn reconcile task — detects dangling sessions and duplicate bots every 60 seconds
    let tl_for_reconcile = tl_service.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            tl_for_reconcile.reconcile().await;
            tl_for_reconcile.evict_duplicates().await;
        }
    });

    let svc = TrafficLightServiceImpl { tl: tl_service, ae_registry };

    // Start JSON-RPC HTTP listener
    let server = Server::builder().build(config.rpc_addr).await?;
    info!("RPC server listening on {}", config.rpc_addr);

    telemetry.healthy();
    info!("Traffic Light is ready");

    let handle = server.start(svc.into_rpc());
    handle.stopped().await;

    Ok(())
}
