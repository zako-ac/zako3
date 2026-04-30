use anyhow::Result;
use chrono::Utc;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};
use zako3_states::{RedisCacheRepository, TapHubStateService, TapMetricsStateService};

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
        .ok(); // Ignore error if already a hypertable or if timescale not available (though expected)

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_tap_metrics_tap_id_time ON tap_metrics (tap_id, time DESC);",
    )
    .execute(&pool)
    .await?;

    info!("Connecting to Redis...");
    let cache_repo: std::sync::Arc<dyn zako3_states::CacheRepository> =
        std::sync::Arc::new(RedisCacheRepository::new(&redis_url).await?);
    let metrics_service = TapMetricsStateService::new(std::sync::Arc::clone(&cache_repo));
    let hub_state_service = TapHubStateService::new(std::sync::Arc::clone(&cache_repo));

    // Spawn the background delta-accumulator task: subscribes to history pubsub
    // and increments per-tap delta hashes in Redis.
    let redis_url_clone = redis_url.clone();
    let delta_client =
        redis::Client::open(redis_url.clone()).expect("Redis client for delta");
    let _pubsub_handle = {
        let delta_client = delta_client.clone();
        tokio::spawn(async move {
            let pubsub = zako3_states::RedisPubSub::new(&redis_url_clone)
                .await
                .expect("RedisPubSub for delta");
            let mut conn_mgr: redis::aio::ConnectionManager = delta_client
                .get_connection_manager()
                .await
                .expect("delta conn mgr");
            use futures_util::StreamExt;
            match pubsub.subscribe_history().await {
                Ok(stream) => {
                    let mut stream = Box::pin(stream);
                    while let Some(entry) = stream.next().await {
                        if let zako3_types::hq::history::UseHistoryEntry::PlayAudio(ref h) = entry {
                            let key = format!("delta_metrics:{}", h.tap_id.0);
                            use redis::AsyncCommands;
                            let _: Result<i64, _> =
                                conn_mgr.hincr(&key, "total_uses", 1i64).await;
                            if h.cache_hit {
                                let _: Result<i64, _> =
                                    conn_mgr.hincr(&key, "cache_hits", 1i64).await;
                            }
                        }
                    }
                }
                Err(e) => tracing::error!(%e, "Failed to subscribe to history"),
            }
        })
    };

    info!("Starting sync loop with interval {}s", sync_interval);

    loop {
        match metrics_service.get_known_taps().await {
            Ok(taps) => {
                let now = Utc::now();
                for tap_id in &taps {
                    // a. Read and zero out delta hash
                    let delta_key = format!("delta_metrics:{}", tap_id.0);
                    let deltas = cache_repo.hgetall(&delta_key).await.unwrap_or_default();
                    let _ = cache_repo.hdel_key(&delta_key).await;

                    let delta_total: i64 = deltas
                        .iter()
                        .find(|(k, _)| k == "total_uses")
                        .and_then(|(_, v)| v.parse().ok())
                        .unwrap_or(0);
                    let delta_cache: i64 = deltas
                        .iter()
                        .find(|(k, _)| k == "cache_hits")
                        .and_then(|(_, v)| v.parse().ok())
                        .unwrap_or(0);

                    // b. Fetch latest TimescaleDB row for this tap
                    let last = sqlx::query(
                        "SELECT total_uses, cache_hits FROM tap_metrics \
                         WHERE tap_id = $1 ORDER BY time DESC LIMIT 1",
                    )
                    .bind(&tap_id.0)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or(None);

                    let last_total: i64 = last.as_ref().map(|r| r.get("total_uses")).unwrap_or(0);
                    let last_cache: i64 = last.as_ref().map(|r| r.get("cache_hits")).unwrap_or(0);

                    let new_total = last_total + delta_total;
                    let new_cache = last_cache + delta_cache;

                    // c. Gauges: read directly
                    let active_now = hub_state_service
                        .get_online_count(tap_id)
                        .await
                        .unwrap_or(0) as i64;
                    let unique_users = metrics_service
                        .get_unique_users_count(tap_id.clone())
                        .await
                        .unwrap_or(0) as i64;

                    // d. Insert new row
                    if let Err(e) = sqlx::query(
                        "INSERT INTO tap_metrics (time, tap_id, total_uses, active_now, cache_hits, unique_users) \
                         VALUES ($1, $2, $3, $4, $5, $6)",
                    )
                    .bind(now)
                    .bind(&tap_id.0)
                    .bind(new_total)
                    .bind(active_now)
                    .bind(new_cache)
                    .bind(unique_users)
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
