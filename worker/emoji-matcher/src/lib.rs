pub mod config;
pub mod constants;
pub mod handlers;
pub mod http;
pub mod logging;
pub mod metrics;
pub mod store;
pub mod types;
pub mod utils;

use crate::config::AppConfig;
use crate::handlers::match_nats::start_match_handler;
use crate::handlers::register_nats::start_register_handler;
use crate::http::{IS_HEALTHY, spawn_http_server};
use crate::store::PgEmojiStore;
use std::sync::{Arc, atomic::Ordering};

pub async fn run() -> anyhow::Result<()> {
    let config = AppConfig::load();
    logging::init_logging(config.otlp_endpoint.clone());

    spawn_http_server(&config.http_addr).await;

    tracing::info!("Connecting to NATS at {}", config.nats_url);
    let client = Arc::new(async_nats::connect(&config.nats_url).await?);
    tracing::info!("Connected to NATS.");

    tracing::info!("Initializing emoji store.");
    let emoji_store = Arc::new(PgEmojiStore::new(&config.database_url).await?);
    tracing::info!("Emoji store initialized.");

    start_register_handler(client.clone(), emoji_store.clone()).await?;
    start_match_handler(client.clone(), emoji_store.clone()).await?;

    IS_HEALTHY.store(true, Ordering::Relaxed);
    tracing::info!("App started.");

    // Keep the main thread alive
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down.");

    Ok(())
}
