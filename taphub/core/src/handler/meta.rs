use std::sync::Arc;

use chrono::Utc;
use hex;
use sha2::Digest;
use zako3_preload_cache::AudioCache;
use zako3_types::{
    AudioMetaResponse, AudioRequest,
    cache::{AudioCacheItem, AudioCacheItemKey},
};

use crate::hub::TapHub;

pub(crate) async fn handle_request_audio_meta_inner(
    tap_hub: &TapHub,
    req: AudioRequest,
) -> Result<AudioMetaResponse, String> {
    let tap_id = tap_hub
        .state_service
        .get_tap_id_by_name(&req.tap_name)
        .await
        .map_err(|e| format!("Failed to get tap id: {}", e))?
        .ok_or_else(|| "Tap disconnected or not found".to_string())?;

    let tap = tap_hub
        .app
        .hq_repository
        .get_tap_by_id(&tap_id.0.to_string())
        .await
        .ok_or_else(|| "Tap metadata not found".to_string())?;

    super::permission::verify_permission(tap_hub, &tap, &req.discord_user_id).await?;

    // Check cache for metadata
    let meta_hash = hex::encode(sha2::Sha256::digest(req.request.to_string().as_bytes()));
    let meta_key = AudioCacheItemKey::ARHash(meta_hash);
    if let Some(entry) = tap_hub.audio_cache.get_entry(&tap_id, &meta_key).await {
        tracing::info!("Metadata cache hit for tap_id={}", tap_id.0);

        return Ok(AudioMetaResponse {
            metadatas: entry.metadatas,
            cache_key: entry.cache_key,
            base_volume: tap.base_volume,
        });
    }

    // Request metadata from connection
    let (connection_id, _disconnect_rx) = tap_hub.select_connection(&tap_id).await?;

    let meta = tap_hub
        .zf_hub
        .request_audio_metadata(
            tap_id.clone(),
            connection_id,
            req.request.clone(),
            Default::default(),
        )
        .await
        .map_err(|e| format!("Failed to request audio metadata from tap: {}", e))?;

    // Write metadata back to cache
    let meta_item = AudioCacheItem {
        key: meta_key,
        tap_id: tap_id.clone(),
        expire_at: meta
            .cache
            .ttl_seconds
            .map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64)),
    };
    {
        let cache = Arc::clone(&tap_hub.audio_cache);
        let metadatas = meta.metadatas.clone();
        let cache_key = meta.cache.clone();
        tokio::spawn(async move {
            if let Err(e) = cache.store_metadata(meta_item, metadatas, cache_key).await {
                tracing::warn!(%e, "Failed to store metadata in cache");
            }
        });
    }

    Ok(AudioMetaResponse {
        metadatas: meta.metadatas,
        cache_key: meta.cache,
        base_volume: tap.base_volume,
    })
}
