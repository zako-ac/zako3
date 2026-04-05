use hq_backend::rpc::start_rpc_server;
use hq_core::{get_pool, run_migrations, AppConfig, Service};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().init();

    info!("Starting hq-boot...");

    let config = Arc::new(AppConfig::load()?);
    let pool = get_pool(&config.database_url).await?;

    run_migrations(&pool).await?;

    let service = Service::new(pool, config.clone()).await?;

    let backend_address = config.backend_address.clone();
    let rpc_address = config.rpc_address.clone();

    let service_backend = service.clone();
    let backend_task = tokio::spawn(async move {
        let app = hq_backend::app(service_backend);

        let listener = TcpListener::bind(&backend_address)
            .await
            .expect("Failed to bind backend port");
        info!("Backend listening on {}", backend_address);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Backend error: {}", e);
        }
    });

    let service_rpc = service.clone();
    let rpc_task = tokio::spawn(async move {
        let rpc = start_rpc_server(service_rpc.api_key, service_rpc.tap, &rpc_address);
        if let Err(e) = rpc.await {
            tracing::error!("RPC server error: {}", e);
        }
    });

    let service_bot = service.clone();
    let bot_task = tokio::spawn(async move {
        info!("Starting bot...");
        if let Err(e) = hq_bot::run(service_bot).await {
            tracing::error!("Bot error: {}", e);
        }
    });

    let _ = tokio::join!(backend_task, bot_task, rpc_task);

    Ok(())
}
