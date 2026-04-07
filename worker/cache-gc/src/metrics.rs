use anyhow::Result;

pub struct ActionMetrics {
    pub action: &'static str,
    /// Number of items examined.
    pub processing_count: u64,
    /// Number of items removed.
    pub evict_count: u64,
    /// Wall-clock duration of the action in milliseconds.
    pub processing_time_ms: u64,
}

impl ActionMetrics {
    pub fn log(&self) {
        tracing::info!(
            action = self.action,
            processing_count = self.processing_count,
            evict_count = self.evict_count,
            processing_time_ms = self.processing_time_ms,
            "cache-gc action complete"
        );
    }
}

/// Persist metrics to Redis if a repository is provided.
///
/// Keys written:
/// - `cache_gc:{action}:processing_count`  — incrby (cumulative)
/// - `cache_gc:{action}:evict_count`        — incrby (cumulative)
/// - `cache_gc:{action}:last_run_time_ms`   — set (latest only)
/// - `cache_gc:{action}:last_run_at`        — set (unix timestamp of latest run)
pub async fn persist(
    metrics: &ActionMetrics,
    cache_repo: &dyn zako3_states::CacheRepository,
) -> Result<()> {
    let a = metrics.action;

    cache_repo
        .incrby(
            &format!("cache_gc:{a}:processing_count"),
            metrics.processing_count as i64,
        )
        .await?;

    cache_repo
        .incrby(
            &format!("cache_gc:{a}:evict_count"),
            metrics.evict_count as i64,
        )
        .await?;

    cache_repo
        .set(
            &format!("cache_gc:{a}:last_run_time_ms"),
            &metrics.processing_time_ms.to_string(),
        )
        .await;

    cache_repo
        .set(
            &format!("cache_gc:{a}:last_run_at"),
            &chrono::Utc::now().timestamp().to_string(),
        )
        .await;

    Ok(())
}
