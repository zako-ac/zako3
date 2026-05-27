pub mod config;
pub mod constants;
pub mod handlers;
pub mod http;
pub mod logging;
pub mod metrics;
pub mod store;
pub mod task_queue;
pub mod types;
pub mod utils;

use std::sync::{Arc, atomic::Ordering};

use sqlx::postgres::PgPoolOptions;
use zako3_states::{RedisCacheRepository, UserSettingsStateService};

use crate::config::AppConfig;
use crate::handlers::scope_match::ScopeMatchContext;
use crate::handlers::scope_match_nats::start_scope_match_handler;
use crate::http::{IS_HEALTHY, spawn_http_server};
use crate::store::{PgHashCache, PgSettingsStore};
use crate::task_queue::TaskQueue;

pub async fn run() -> anyhow::Result<()> {
    let config = Arc::new(AppConfig::load());
    logging::init_logging(config.otlp_endpoint.clone());

    spawn_http_server(&config.http_addr).await;

    tracing::info!("Connecting to NATS at {}", config.nats_url);
    let client = Arc::new(async_nats::connect(&config.nats_url).await?);
    tracing::info!("Connected to NATS.");

    tracing::info!("Connecting to Postgres.");
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect(&config.database_url)
        .await?;

    let hash_cache = Arc::new(PgHashCache::new(pool.clone()).await?);
    let settings_store = Arc::new(PgSettingsStore::new(pool));

    tracing::info!("Connecting to Redis at {}", config.redis_url);
    let redis_repo = Arc::new(RedisCacheRepository::new(&config.redis_url).await?);
    let cache_invalidator = UserSettingsStateService::new(redis_repo);

    let ctx = Arc::new(ScopeMatchContext {
        config: config.clone(),
        hash_cache,
        settings: settings_store,
        cache_invalidator,
    });

    let queue = TaskQueue::spawn(ctx, config.worker_concurrency, config.queue_capacity);

    start_scope_match_handler(client, queue).await?;

    IS_HEALTHY.store(true, Ordering::Relaxed);
    tracing::info!("App started.");

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down.");

    Ok(())
}
