use anyhow::{Context, Result};
use tokio::fs;
use zako3_preload_cache::{AudioCache, FileAudioCache};
use zako3_types::cache::AudioCacheItemKey;
use zako3_types::hq::TapId;

use crate::metrics::ActionMetrics;

/// Evict cache entries by GDSF priority until total opus file size is under `max_bytes`.
///
/// Algorithm:
/// 1. Refresh GDSF priorities for all complete entries based on current use_count / file_size.
/// 2. Sum total disk usage.
/// 3. Evict lowest-priority entries in batches until under budget.
pub async fn evict_gdsf(
    cache: &FileAudioCache,
    max_bytes: u64,
    batch_size: usize,
) -> Result<ActionMetrics> {
    let started = std::time::Instant::now();
    let mut evict_count = 0u64;

    // Step 1: collect all complete entries and refresh priorities.
    let entries = cache.db().get_all_entries().await?;
    let mut file_sizes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut total_bytes: u64 = 0;
    let clock = 0.0f64;

    for entry in &entries {
        let Some(ref opus_path) = entry.opus_path else { continue };
        if entry.is_downloading {
            continue;
        }
        let size = match fs::metadata(opus_path).await {
            Ok(m) => m.len(),
            Err(_) => continue, // missing file will be caught by evict_dangling
        };
        file_sizes.insert(opus_path.clone(), size);
        total_bytes += size;

        let priority = clock + if size > 0 { entry.use_count as f64 / size as f64 } else { 0.0 };
        let tap_id = TapId(entry.tap_id.clone());
        let key: AudioCacheItemKey = match serde_json::from_str(&entry.cache_key) {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(%e, tap_id = %tap_id.0, "failed to deserialize cache key, skipping");
                continue;
            }
        };
        if let Err(e) = cache.update_gdsf_priority(&tap_id, &key, priority).await {
            tracing::warn!(%e, tap_id = %tap_id.0, "failed to update GDSF priority");
        }
    }

    let processing_count = file_sizes.len() as u64;
    tracing::info!(total_bytes, max_bytes, processing_count, "GDSF: starting eviction pass");

    // Step 2: evict until under budget.
    loop {
        if total_bytes <= max_bytes {
            break;
        }

        let candidates = cache
            .eviction_candidates(batch_size)
            .await
            .context("failed to fetch eviction candidates")?;

        if candidates.is_empty() {
            tracing::warn!("no eviction candidates left; cache is still over budget");
            break;
        }

        for (item, _priority) in candidates {
            if total_bytes <= max_bytes {
                break;
            }

            // Look up the file size we recorded earlier (by tap_id+key lookup via DB).
            let entry = cache.db().get(item.tap_id.to_string(), {
                serde_json::to_string(&item.key).unwrap_or_default()
            }).await;

            let file_size = entry
                .ok()
                .flatten()
                .and_then(|e| e.opus_path)
                .and_then(|p| file_sizes.get(&p).copied())
                .unwrap_or(0);

            match cache.delete(&item.tap_id, &item.key).await {
                Ok(()) => {
                    total_bytes = total_bytes.saturating_sub(file_size);
                    evict_count += 1;
                    tracing::debug!(
                        tap_id = %item.tap_id.0,
                        key = %item.key,
                        file_size,
                        total_bytes,
                        "evicted entry"
                    );
                }
                Err(e) => {
                    tracing::warn!(%e, tap_id = %item.tap_id.0, "failed to delete entry");
                }
            }
        }

    }

    Ok(ActionMetrics {
        action: "evict_gdsf",
        processing_count,
        evict_count,
        processing_time_ms: started.elapsed().as_millis() as u64,
    })
}
