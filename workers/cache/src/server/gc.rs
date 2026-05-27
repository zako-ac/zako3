use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use zako3_preload_cache::FileAudioCache;
use zako3_states::RedisCacheRepository;

use crate::{actions, metrics};

pub struct GcConfig {
    pub interval: Duration,
    pub max_bytes: Option<u64>,
    pub batch_size: usize,
}

/// Spawn the background GC loop. Each tick runs:
/// `evict_expired` → `evict_dangling` → `evict_gdsf` → `validate_opus`,
/// matching the previous `run-all` behavior.
pub fn spawn(
    cfg: GcConfig,
    cache: Arc<FileAudioCache>,
    cache_dir: std::path::PathBuf,
    repo: Option<Arc<RedisCacheRepository>>,
) {
    tokio::spawn(async move {
        // Skew the first run by `interval / 4` so the server has time to warm up.
        let warmup = cfg.interval / 4;
        tokio::time::sleep(warmup).await;
        let mut ticker = tokio::time::interval(cfg.interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            run_once(&cfg, &cache, &cache_dir, repo.as_deref()).await;
        }
    });
}

async fn run_once(
    cfg: &GcConfig,
    cache: &FileAudioCache,
    cache_dir: &Path,
    repo: Option<&RedisCacheRepository>,
) {
    match actions::expired::evict_expired(cache).await {
        Ok(m) => finish(m, repo).await,
        Err(e) => tracing::warn!(%e, "evict_expired failed"),
    }
    match actions::dangling::evict_dangling(cache, cache_dir).await {
        Ok(m) => finish(m, repo).await,
        Err(e) => tracing::warn!(%e, "evict_dangling failed"),
    }
    if let Some(max_bytes) = cfg.max_bytes {
        match actions::gdsf::evict_gdsf(cache, max_bytes, cfg.batch_size).await {
            Ok(m) => finish(m, repo).await,
            Err(e) => tracing::warn!(%e, "evict_gdsf failed"),
        }
    }
    match actions::validate::validate_opus(cache).await {
        Ok(m) => finish(m, repo).await,
        Err(e) => tracing::warn!(%e, "validate_opus failed"),
    }
}

async fn finish(m: metrics::ActionMetrics, repo: Option<&RedisCacheRepository>) {
    m.log();
    if let Some(repo) = repo
        && let Err(e) = metrics::persist(&m, repo).await
    {
        tracing::warn!(%e, "failed to persist metrics to Redis");
    }
}
