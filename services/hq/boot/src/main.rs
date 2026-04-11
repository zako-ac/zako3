use futures_util::StreamExt;
use hq_backend::rpc::start_rpc_server;
use hq_core::{get_pool, run_migrations, AppConfig, PlaybackEvent, Service};
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

    // Subscribe to NATS tap_used events and bridge to stats_tx.
    if let Some(ref url) = config.nats_url {
        let stats_tx2 = stats_tx.clone();
        let nats = async_nats::connect(url.as_str()).await?;
        let mut sub = nats.subscribe("zako3.stats.tap_used").await?;
        tokio::spawn(async move {
            while sub.next().await.is_some() {
                let _ = stats_tx2.send(());
            }
        });
        info!("Subscribed to NATS zako3.stats.tap_used at {}", url);
    } else {
        info!("NATS_URL not set; stats SSE will not receive live updates");
    }

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
