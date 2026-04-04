use hq_core::{AppConfig, Service, get_pool};
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
    let service = Service::new(pool, config.clone()).await?;

    let service_backend = service.clone();
    let backend_task = tokio::spawn(async move {
        let app = hq_backend::app(service_backend);
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr)
            .await
            .expect("Failed to bind backend port");
        info!("Backend listening on {}", addr);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Backend error: {}", e);
        }
    });

    let service_bot = service.clone();
    let bot_task = tokio::spawn(async move {
        info!("Starting bot...");
        if let Err(e) = hq_bot::run(service_bot).await {
            tracing::error!("Bot error: {}", e);
        }
    });

    let _ = tokio::join!(backend_task, bot_task);

    Ok(())
}
