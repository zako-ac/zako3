use std::sync::Arc;

use bytes::Bytes;
use hex;
use protofish2::Timestamp;
use sha2::Digest;
use tokio::sync::mpsc;
use zako3_preload_cache::{AudioCache, NextFrame};
use zako3_types::{AudioMetaResponse, CachedAudioRequest, hq::UserId};

use crate::hub::TapHub;

use super::{cache::build_cache_item, cache::resolve_metadata, stream::bridge_rel};

pub(crate) async fn handle_request_audio_inner(
    tap_hub: &TapHub,
    request: CachedAudioRequest,
) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
    // Get tap ID from name
    let tap_id = tap_hub
        .state_service
        .get_tap_id_by_name(&request.tap_name)
        .await
        .map_err(|e| format!("Failed to get tap id: {}", e))?
        .ok_or_else(|| "Tap disconnected or not found".to_string())?;

    // Record metrics
    if let Err(e) = tap_hub.metrics_service.inc_total_uses(tap_id.clone()).await {
        tracing::warn!(%e, "Failed to increment total_uses metric");
    }
    if let Err(e) = tap_hub
        .metrics_service
        .record_unique_user(tap_id.clone(), UserId(request.discord_user_id.0.clone()))
        .await
    {
        tracing::warn!(%e, "Failed to record unique_user metric");
    }

    let tap = tap_hub
        .app
        .hq_repository
        .get_tap_by_id(&tap_id.0.to_string())
        .await
        .ok_or_else(|| "Tap metadata not found".to_string())?;

    super::permission::verify_permission(tap_hub, &tap, &request.discord_user_id).await?;

    // Try cache hit
    let cache_item = build_cache_item(tap_id.clone(), &request.cache_key, &request.audio_request);

    if let Some(ref item) = cache_item {
        if let Some(entry) = tap_hub.audio_cache.get_entry(&item.tap_id, &item.key).await {
            if entry.has_audio() && !entry.is_downloading() {
                tracing::info!("Cache hit for tap_id={}, key={}", item.tap_id.0, item.key);
                if let Some(reader) = tap_hub
                    .audio_cache
                    .open_reader(&item.tap_id, &item.key)
                    .await
                {
                    tap_hub
                        .metrics_service
                        .inc_cache_hits(tap_id.clone())
                        .await
                        .ok();

                    let (tx, rx) = mpsc::channel(100);
                    tokio::spawn(async move {
                        let mut reader = reader;
                        let mut frame_count = 0u64;
                        loop {
                            match reader.next_frame().await {
                                Ok(NextFrame::Frame(bytes)) => {
                                    let ts = Timestamp(frame_count * 20);
                                    frame_count += 1;
                                    if tx.send((ts, bytes)).await.is_err() {
                                        break;
                                    }
                                }
                                Ok(NextFrame::Pending) | Ok(NextFrame::Done) | Err(_) => break,
                            }
                        }
                    });
                    let meta = AudioMetaResponse {
                        metadatas: entry.metadatas,
                        cache_key: entry.cache_key,
                        base_volume: tap.base_volume,
                    };
                    return Ok((meta, rx));
                } else {
                    tracing::warn!(
                        "Cache entry found but failed to open reader for tap_id={}, key={}",
                        item.tap_id.0,
                        item.key
                    );
                }
            }
        }
    }

    tracing::info!(
        "Cache miss for tap_id={}, cache_key={:?}",
        tap_id.0,
        cache_item.as_ref().map(|i| &i.key)
    );

    // Cache miss: request from zakofish
    let (connection_id, disconnect_rx) = tap_hub.select_connection(&tap_id).await?;

    let (succ, rel, mut unrel) = tap_hub
        .zf_hub
        .request_audio(
            tap_id.clone(),
            connection_id,
            request.audio_request.clone(),
            Default::default(),
        )
        .await
        .map_err(|e| format!("Failed to request audio from tap: {}", e))?;

    tracing::info!("Received audio from zakofish for tap_id={}", tap_id.0);

    // Bridge reliable stream
    let (rel_rx, done_rx) = bridge_rel(rel, disconnect_rx);

    // Resolve metadata
    let metadatas = resolve_metadata(
        tap_hub,
        succ.metadatas,
        &tap_id,
        &request.audio_request.to_string(),
    )
    .await;

    if let Some(item) = cache_item {
        let cache = Arc::clone(&tap_hub.audio_cache);
        let metadatas_clone = metadatas.clone();
        let cache_key = succ.cache.clone();
        let item_clone = item.clone();

        tokio::spawn(async move {
            if let Err(e) = cache
                .store(item_clone, metadatas_clone, cache_key, rel_rx, done_rx)
                .await
            {
                tracing::warn!(%e, "Failed to store audio in cache");
            }
        });

        // If audio was cached under a CacheKey, also store metadata under ARHash
        // so that metadata-only requests can find it regardless of cache policy.
        if matches!(item.key, zako3_types::cache::AudioCacheItemKey::CacheKey(_)) {
            let meta_hash = hex::encode(sha2::Sha256::digest(
                request.audio_request.to_string().as_bytes(),
            ));
            let meta_item = zako3_types::cache::AudioCacheItem {
                key: zako3_types::cache::AudioCacheItemKey::ARHash(meta_hash),
                tap_id: tap_id.clone(),
                expire_at: item.expire_at,
            };
            let cache2 = Arc::clone(&tap_hub.audio_cache);
            let metadatas2 = metadatas.clone();
            let cache_key2 = succ.cache.clone();
            tokio::spawn(async move {
                if let Err(e) = cache2
                    .store_metadata(meta_item, metadatas2, cache_key2)
                    .await
                {
                    tracing::warn!(%e, "Failed to store ARHash metadata alias in cache");
                }
            });
        }
    }

    let meta = AudioMetaResponse {
        metadatas,
        cache_key: succ.cache,
        base_volume: tap.base_volume,
    };

    let (tx, rx) = mpsc::channel(100);
    tokio::spawn(async move {
        while let Some(chunk) = unrel.recv().await {
            if let Err(e) = tx.send((chunk.timestamp, chunk.content)).await {
                tracing::warn!(%e, "Failed to send audio chunk to channel");
                return;
            }
        }
    });

    Ok((meta, rx))
}
