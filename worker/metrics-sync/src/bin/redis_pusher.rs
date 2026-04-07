use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tracing::info;
use zako3_states::{CacheRepository, RedisCacheRepository};
use zako3_types::hq::TapId;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;

    info!("Connecting to Redis...");
    let cache_repo = std::sync::Arc::new(RedisCacheRepository::new(&redis_url).await?);

    info!("Fetching latest snapshots from TimescaleDB...");
    // Using query_as! or query! requires DATABASE_URL at compile time.
    // For now we use the non-macro version to ensure it compiles in this environment.
    let latest_snapshots = sqlx::query(
        "SELECT DISTINCT ON (tap_id) tap_id, total_uses, cache_hits
         FROM tap_metrics
         ORDER BY tap_id, time DESC",
    )
    .fetch_all(&pool)
    .await?;

    for row in latest_snapshots {
        use sqlx::Row;
        let tap_id_val: String = row.get("tap_id");
        let tap_id = TapId(tap_id_val);
        info!("Restoring metrics for tap {}...", tap_id.0);

        let total_uses: i64 = row.get("total_uses");
        let cache_hits: i64 = row.get("cache_hits");

        let metrics = [
            ("total_uses", total_uses),
            ("cache_hits", cache_hits),
        ];

        for (key_name, val) in metrics {
            let redis_key = format!("metrics:{}:{}", tap_id.0, key_name);
            cache_repo.set(&redis_key, &val.to_string()).await;
        }

        // Add to known taps set
        cache_repo
            .sadd("metrics:known_taps", &tap_id.0.to_string())
            .await?;
    }

    info!("Redis restoration complete.");
    Ok(())
}
