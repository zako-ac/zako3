use std::net::SocketAddr;
use std::sync::Arc;

use futures::StreamExt as _;
use serde::Deserialize;
use tarpc::{
    context::Context,
    server::{BaseChannel, Channel},
    tokio_serde::formats::Json,
};
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, TrafficLight};
use zako3_types::{GuildId, SessionState};
use tokio::sync::RwLock;
use tracing::info;
use zako3_tl_core::{
    DiscordToken, TlService, Worker, WorkerPermissions, WorkerId, ZakoState,
};
use zako3_tl_infra::AeRegistry;
use zako3_telemetry::TelemetryConfig;
use zako3_types::hq::DiscordUserId;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AppConfig {
    // Telemetry
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    // tarpc listener (callers connect here)
    #[serde(default = "default_tarpc_addr")]
    pub tarpc_addr: SocketAddr,

    // ae-transport listener (AEs connect here)
    #[serde(default = "default_ae_transport_addr")]
    pub ae_transport_addr: SocketAddr,

    // Comma-separated Discord bot tokens, e.g. "tokenA,tokenB,tokenC"
    pub discord_tokens: String,
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_tarpc_addr() -> SocketAddr {
    "0.0.0.0:7070".parse().unwrap()
}

fn default_ae_transport_addr() -> SocketAddr {
    "0.0.0.0:7071".parse().unwrap()
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
// tarpc service impl
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TrafficLightServiceImpl {
    tl: Arc<TlService>,
}

impl TrafficLight for TrafficLightServiceImpl {
    async fn execute(
        self,
        _ctx: Context,
        request: AudioEngineCommandRequest,
    ) -> AudioEngineCommandResponse {
        self.tl.execute(request).await
    }

    async fn get_sessions_in_guild(self, _ctx: Context, guild_id: GuildId) -> Vec<SessionState> {
        self.tl.get_sessions_in_guild(guild_id).await
    }

    async fn report_guilds(self, _ctx: Context, token: String, guilds: Vec<GuildId>) {
        self.tl.report_guilds(token, guilds).await
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

    // Build AE registry (TlServer that AEs connect to)
    let state = Arc::new(RwLock::new(initial_state));
    let ae_registry = Arc::new(
        AeRegistry::new(config.ae_transport_addr, state, tokens).await?,
    );
    info!("AE transport listening on {}", config.ae_transport_addr);

    // Spawn the AE accept loop
    let registry_for_loop = ae_registry.clone();
    tokio::spawn(async move {
        registry_for_loop.accept_loop().await;
    });

    // Build TlService backed by the AE registry — shares the same state Arc so
    // accept_loop writes (connected_ae_ids) are immediately visible to the router.
    let tl_service = Arc::new(TlService::new(ae_registry.state(), ae_registry.clone()));

    // Start tarpc listener
    let mut listener =
        tarpc::serde_transport::tcp::listen(&config.tarpc_addr, Json::default).await?;
    info!("tarpc listening on {}", config.tarpc_addr);

    telemetry.healthy();
    info!("Traffic Light is ready");

    let svc = TrafficLightServiceImpl { tl: tl_service };

    while let Some(conn) = listener.next().await {
        let transport = match conn {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("tarpc accept error: {e}");
                continue;
            }
        };
        let svc = svc.clone();
        tokio::spawn(async move {
            BaseChannel::with_defaults(transport)
                .execute(svc.serve())
                .for_each_concurrent(None, |fut| fut)
                .await;
        });
    }

    Ok(())
}
