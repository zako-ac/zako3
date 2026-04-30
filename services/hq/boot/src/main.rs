use hq_backend::rpc::start_rpc_server;
use hq_core::{AppConfig, PlaybackEvent, Service, get_pool, run_migrations};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::info;

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
    let timescale_pool = get_pool(&config.timescale_database_url).await?;

    run_migrations(&pool).await?;

    // Broadcast channel for playback state events.
    let (event_tx, _) = broadcast::channel::<PlaybackEvent>(128);

    let service = Service::new(pool, timescale_pool, config.clone(), event_tx.clone()).await?;

    // Broadcast channel for stats events — fired whenever a tap processes an audio request.
    let (stats_tx, _) = broadcast::channel::<()>(128);

    // Bridge Redis history channel to stats_tx for SSE.
    let stats_tx2 = stats_tx.clone();
    let redis_url2 = config.redis_url.clone();
    tokio::spawn(async move {
        match zako3_states::RedisPubSub::new(&redis_url2).await {
            Ok(pubsub) => match pubsub.subscribe_history().await {
                Ok(stream) => {
                    use futures_util::StreamExt;
                    let mut stream = Box::pin(stream);
                    while stream.next().await.is_some() {
                        let _ = stats_tx2.send(());
                    }
                }
                Err(e) => {
                    tracing::error!(%e, "Failed to subscribe to Redis history channel for stats")
                }
            },
            Err(e) => tracing::error!(%e, "Failed to connect Redis PubSub for stats bridge"),
        }
    });
    info!("Stats SSE bridged from Redis history channel");

    let backend_address = config.backend_address.clone();
    let service_backend = service.clone();
    let event_tx_backend = event_tx.clone();
    let stats_tx_backend = stats_tx.clone();
    let backend_task = tokio::spawn(async move {
        let app = hq_backend::app(service_backend, event_tx_backend, stats_tx_backend);

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
