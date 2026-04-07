use anyhow::Result;
use zako3_preload_cache::{AudioCache, FileAudioCache};
use zako3_types::cache::AudioCacheItemKey;
use zako3_types::hq::TapId;

use crate::metrics::ActionMetrics;

/// Remove all entries whose `expire_at` is in the past.
pub async fn evict_expired(cache: &FileAudioCache) -> Result<ActionMetrics> {
    let started = std::time::Instant::now();
    let now = chrono::Utc::now().timestamp();

    let entries = cache.db().get_all_entries().await?;
    let mut processing_count = 0u64;
    let mut evict_count = 0u64;

    for entry in entries {
        let Some(expire_at) = entry.expire_at else {
            continue;
        };
        processing_count += 1;
        if expire_at <= now {
            let tap_id = TapId(entry.tap_id);
            let key: AudioCacheItemKey = match serde_json::from_str(&entry.cache_key) {
                Ok(k) => k,
                Err(e) => {
                    tracing::warn!(%e, tap_id = %tap_id.0, "failed to deserialize cache key, skipping");
                    continue;
                }
            };
            match cache.delete(&tap_id, &key).await {
                Ok(()) => evict_count += 1,
                Err(e) => tracing::warn!(%e, tap_id = %tap_id.0, "failed to delete expired entry"),
            }
        }
    }

    Ok(ActionMetrics {
        action: "evict_expired",
        processing_count,
        evict_count,
        processing_time_ms: started.elapsed().as_millis() as u64,
    })
}
