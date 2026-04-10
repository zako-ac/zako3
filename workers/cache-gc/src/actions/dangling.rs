use std::{collections::HashSet, path::Path};

use anyhow::Result;
use tokio::fs;
use zako3_preload_cache::{AudioCache, FileAudioCache};
use zako3_types::cache::AudioCacheItemKey;
use zako3_types::hq::TapId;

use crate::metrics::ActionMetrics;

/// Remove orphan files (no DB entry) and ghost DB rows (missing file).
///
/// Orphan `.opus` files and their sibling `.json` sidecars are removed together.
/// Orphan `.json` files (no corresponding DB `json_path`) are also removed.
/// Lock files (*.lock) are skipped — they are managed by AudioPreload in taphub.
pub async fn evict_dangling(cache: &FileAudioCache, cache_dir: &Path) -> Result<ActionMetrics> {
    let started = std::time::Instant::now();
    let mut processing_count = 0u64;
    let mut evict_count = 0u64;

    // Pass 1: orphan files — files in the directory with no DB entry.
    let known_opus_paths: HashSet<String> = cache
        .db()
        .get_all_opus_paths()
        .await?
        .into_iter()
        .collect();

    let known_json_paths: HashSet<String> = cache
        .db()
        .get_all_json_paths()
        .await?
        .into_iter()
        .collect();

    let mut dir = fs::read_dir(cache_dir).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        let Some(ext) = path.extension() else { continue };

        if ext == "opus" {
            processing_count += 1;
            let path_str = path.to_string_lossy().into_owned();
            if !known_opus_paths.contains(&path_str) {
                match fs::remove_file(&path).await {
                    Ok(()) => {
                        tracing::info!(path = %path_str, "removed orphan .opus file");
                        evict_count += 1;
                    }
                    Err(e) => tracing::warn!(%e, path = %path_str, "failed to remove orphan .opus file"),
                }
                // Also remove sibling .json sidecar if present.
                let json_sibling = path.with_extension("json");
                if json_sibling.exists() {
                    let _ = fs::remove_file(&json_sibling).await;
                }
            }
        } else if ext == "json" {
            processing_count += 1;
            let path_str = path.to_string_lossy().into_owned();
            if !known_json_paths.contains(&path_str) {
                match fs::remove_file(&path).await {
                    Ok(()) => {
                        tracing::info!(path = %path_str, "removed orphan .json sidecar");
                        evict_count += 1;
                    }
                    Err(e) => tracing::warn!(%e, path = %path_str, "failed to remove orphan .json sidecar"),
                }
            }
        }
    }

    // Pass 2: ghost DB rows — complete entries whose opus file no longer exists.
    let entries = cache.db().get_all_entries().await?;
    for entry in entries {
        let Some(ref opus_path) = entry.opus_path else { continue };
        if entry.is_downloading {
            continue;
        }
        processing_count += 1;
        match fs::metadata(opus_path).await {
            Ok(_) => {} // file exists
            Err(_) => {
                let tap_id = TapId(entry.tap_id);
                let key: AudioCacheItemKey = match serde_json::from_str(&entry.cache_key) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!(%e, tap_id = %tap_id.0, "failed to deserialize cache key, skipping");
                        continue;
                    }
                };
                match cache.delete(&tap_id, &key).await {
                    Ok(()) => {
                        tracing::info!(tap_id = %tap_id.0, opus_path, "removed ghost DB row");
                        evict_count += 1;
                    }
                    Err(e) => tracing::warn!(%e, tap_id = %tap_id.0, "failed to delete ghost DB row"),
                }
            }
        }
    }

    Ok(ActionMetrics {
        action: "evict_dangling",
        processing_count,
        evict_count,
        processing_time_ms: started.elapsed().as_millis() as u64,
    })
}
