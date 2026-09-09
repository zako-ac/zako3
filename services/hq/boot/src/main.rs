use hq_backend::rpc::start_rpc_server;
use hq_core::{AppConfig, PlaybackEvent, Service, get_pool, run_migrations};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::info;

mod bridge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Arc::new(AppConfig::load()?);

    let telemetry = zako3_telemetry::init(zako3_telemetry::TelemetryConfig {
        service_name: "hq".to_string(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: config.metrics_port,
    })
    .await?;

    info!("Starting hq-boot...");

    let pool = get_pool(&config.database_url).await?;
    let timescale_pool = match &config.timescale_database_url {
        Some(url) => match get_pool(url).await {
            Ok(pool) => Some(pool),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "TimescaleDB unavailable — metrics history features will be disabled"
                );
                None
            }
        },
        None => {
            tracing::warn!(
                "TIMESCALE_DATABASE_URL not set — metrics history features will be disabled"
            );
            None
        }
    };

    run_migrations(&pool).await?;

    // Broadcast channel for playback state events.
    let (event_tx, _) = broadcast::channel::<PlaybackEvent>(128);

    let service = Service::new(pool, timescale_pool, config.clone(), event_tx.clone()).await?;

    // Broadcast channel for stats events — fired whenever a tap processes an audio request.
    let (stats_tx, _) = broadcast::channel::<()>(128);

    // Bridge Redis history channel to both stats_tx and event_tx for SSE.
    tokio::spawn(bridge::run_history_bridge(
        config.redis_url.clone(),
        event_tx.clone(),
        stats_tx.clone(),
    ));
    info!("History bridge started (stats SSE + playback SSE)");

    // Identifies this process in presence entries and NATS subjects. In
    // Kubernetes this is the pod name; the hostname is a reasonable stand-in
    // anywhere else, and a random suffix keeps two local processes distinct.
    let replica_id = std::env::var("POD_NAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| format!("hq-{}", uuid::Uuid::new_v4()));

    // Optional: without NATS the gateway still serves taps, it just cannot
    // reach a connection held by another replica.
    let gateway_nats = match config.nats_url.as_deref() {
        Some(url) => match async_nats::connect(url).await {
            Ok(client) => Some(client),
            Err(e) => {
                tracing::warn!(%e, "NATS connect failed; cross-replica tap dispatch disabled");
                None
            }
        },
        None => None,
    };

    let presence = zako3_states::GatewayPresenceService::new(Arc::new(
        zako3_states::RedisCacheRepository::new(&config.redis_url).await?,
    ));
    let gateway = hq_backend::gateway::Gateway::new(
        service.clone(),
        presence,
        gateway_nats,
        replica_id.clone(),
    );
    info!(replica_id = %replica_id, "Tap gateway ready at /gateway");

    // The audio plane. Every tap answers `Legacy` until it is individually
    // flipped to `gateway_v4`, so wiring this in changes nothing on its own.
    let sink_registry = zako3_states::SinkRegistry::new(Arc::new(
        zako3_states::RedisCacheRepository::new(&config.redis_url).await?,
    ));
    let audio_service = hq_core::service::audio::AudioRequestService::new(
        service.tap.repo(),
        service.tap.clone(),
        service.cache_admin.clone(),
        service.tap_metrics.clone(),
        Arc::new(zako3_states::RedisPubSub::new(&config.redis_url).await?),
        sink_registry,
        Arc::new(hq_backend::gateway::dispatcher::GatewayDispatcher::new(
            gateway.clone(),
        )),
        std::env::var("ZK_UDP_PROXY_ADDR").ok().filter(|s| !s.is_empty()),
    );

    // Opt-in, and the second off-switch for the preload path: without a sink id
    // HQ never asks the cache worker for a ticket and every preload takes the
    // legacy route, whatever a tap's `gateway_v4` flag says.
    let audio_service = match std::env::var("HQ_CACHE_SINK_ID")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(sink_id) => {
            tracing::info!(%sink_id, "direct preload path enabled");
            audio_service.with_cache_ingest(service.cache_admin.clone(), sink_id)
        }
        None => audio_service,
    };

    let backend_address = config.backend_address.clone();
    let service_backend = service.clone();
    let event_tx_backend = event_tx.clone();
    let stats_tx_backend = stats_tx.clone();
    let backend_task = tokio::spawn(async move {
        let app = hq_backend::app_with_gateway(
            service_backend,
            event_tx_backend,
            stats_tx_backend,
            Some(gateway),
        );

        let listener = TcpListener::bind(&backend_address)
            .await
            .expect("Failed to bind backend port");
        info!("Backend listening on {}", backend_address);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Backend error: {}", e);
            panic!("Backend server failed");
        }
    });

    let service_rpc = service.clone();
    let rpc_address = config.rpc_address.clone();
    let rpc_admin_token = config.rpc_admin_token.clone();
    let rpc_task = tokio::spawn(async move {
        let rpc = start_rpc_server(
            service_rpc.api_key,
            service_rpc.tap,
            service_rpc.auth,
            &rpc_address,
            rpc_admin_token,
            Some(audio_service),
        );
        if let Err(e) = rpc.await {
            tracing::error!("RPC server error: {}", e);
            panic!("RPC server failed");
        }
    });

    let service_bot = service.clone();
    let resolver_slot = service.name_resolver_slot.clone();
    let bot_task = tokio::spawn(async move {
        info!("Starting bot...");
        if let Err(e) = hq_bot::run(service_bot, resolver_slot, event_tx.clone()).await {
            tracing::error!("Bot error: {}", e);
            panic!("Bot failed");
        }
    });

    telemetry.healthy();

    let _ = tokio::join!(backend_task, bot_task, rpc_task);

    Ok(())
}
