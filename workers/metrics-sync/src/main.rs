use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};
use zako3_metrics::TapMetricsService;
use zako3_states::{RedisCacheRepository, TapHubStateService};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let otlp_endpoint = env::var("OTLP_ENDPOINT").ok();
    let _telemetry = zako3_telemetry::init(zako3_telemetry::TelemetryConfig {
        service_name: "metrics-sync".to_string(),
        otlp_endpoint,
        metrics_port: None,
    })
    .await?;

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");
    let sync_interval = env::var("SYNC_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<u64>()
        .unwrap_or(60);

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    info!("Initializing TimescaleDB schema...");
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tap_metrics (
            time TIMESTAMPTZ NOT NULL,
            tap_id TEXT NOT NULL,
            total_uses BIGINT DEFAULT 0,
            active_now BIGINT DEFAULT 0,
            cache_hits BIGINT DEFAULT 0,
            unique_users BIGINT DEFAULT 0
        );",
    )
    .execute(&pool)
    .await?;

    sqlx::query("SELECT create_hypertable('tap_metrics', 'time', if_not_exists => TRUE);")
        .execute(&pool)
        .await
        .ok();

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_tap_metrics_tap_id_time ON tap_metrics (tap_id, time DESC);",
    )
    .execute(&pool)
    .await?;

    info!("Connecting to Redis...");
    let cache_repo: std::sync::Arc<dyn zako3_states::CacheRepository> =
        std::sync::Arc::new(RedisCacheRepository::new(&redis_url).await?);
    let metrics_svc = TapMetricsService::new(std::sync::Arc::clone(&cache_repo), pool.clone());
    let hub_state_service = TapHubStateService::new(std::sync::Arc::clone(&cache_repo));

    let redis_url_clone = redis_url.clone();
    let metrics_svc2 = metrics_svc.clone();
    let _pubsub_handle = tokio::spawn(async move {
        let pubsub = zako3_states::RedisPubSub::new(&redis_url_clone)
            .await
            .expect("RedisPubSub for delta");
        use futures_util::StreamExt;
        match pubsub.subscribe_history().await {
            Ok(stream) => {
                let mut stream = Box::pin(stream);
                while let Some(entry) = stream.next().await {
                    if let zako3_types::hq::history::UseHistoryEntry::PlayAudio(ref h) = entry {
                        let _ = metrics_svc2.incr_delta_total_uses(&h.tap_id).await;
                        if h.cache_hit {
                            let _ = metrics_svc2.incr_delta_cache_hits(&h.tap_id).await;
                        }
                    }
                }
            }
            Err(e) => tracing::error!(%e, "Failed to subscribe to history"),
        }
    });

    info!("Starting sync loop with interval {}s", sync_interval);

    loop {
        match metrics_svc.get_known_taps().await {
            Ok(taps) => {
                let now = Utc::now();
                info!("Updating {} histories", taps.len());
                for tap_id in &taps {
                    let active_now = hub_state_service
                        .get_online_count(tap_id)
                        .await
                        .unwrap_or(0) as i64;
                    let unique_users = metrics_svc
                        .get_unique_users_count(tap_id.clone())
                        .await
                        .unwrap_or(0) as i64;
                    if let Err(e) = metrics_svc
                        .sync_tap(tap_id, now, active_now, unique_users)
                        .await
                    {
                        error!("Failed to sync metrics for tap {}: {:?}", tap_id.0, e);
                    }
                }
                info!("Synced metrics for {} taps", taps.len());
            }
            Err(e) => {
                error!("Failed to get known taps: {:?}", e);
            }
        }

        sleep(Duration::from_secs(sync_interval)).await;
    }
}
