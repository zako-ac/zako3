use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};
use zako3_states::{RedisCacheRepository, TapMetricKey, TapMetricsStateService};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

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
        .ok(); // Ignore error if already a hypertable or if timescale not available (though expected)

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tap_metrics_tap_id_time ON tap_metrics (tap_id, time DESC);")
        .execute(&pool)
        .await?;

    info!("Connecting to Redis...");
    let cache_repo = std::sync::Arc::new(RedisCacheRepository::new(&redis_url).await?);
    let metrics_service = TapMetricsStateService::new(cache_repo);

    info!("Starting sync loop with interval {}s", sync_interval);

    loop {
        match metrics_service.get_known_taps().await {
            Ok(taps) => {
                let now = Utc::now();
                for tap_id in &taps {
                    let total_uses = metrics_service
                        .get_metric(tap_id.clone(), TapMetricKey::TotalUses)
                        .await
                        .unwrap_or(0);
                    let active_now = metrics_service
                        .get_metric(tap_id.clone(), TapMetricKey::ActiveNow)
                        .await
                        .unwrap_or(0);
                    let cache_hits = metrics_service
                        .get_metric(tap_id.clone(), TapMetricKey::CacheHits)
                        .await
                        .unwrap_or(0);
                    let unique_users = metrics_service
                        .get_unique_users_count(tap_id.clone())
                        .await
                        .unwrap_or(0);

                    if let Err(e) = sqlx::query(
                        "INSERT INTO tap_metrics (time, tap_id, total_uses, active_now, cache_hits, unique_users)
                         VALUES ($1, $2, $3, $4, $5, $6)"
                    )
                    .bind(now)
                    .bind(tap_id.0.clone())
                    .bind(total_uses as i64)
                    .bind(active_now as i64)
                    .bind(cache_hits as i64)
                    .bind(unique_users as i64)
                    .execute(&pool)
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
