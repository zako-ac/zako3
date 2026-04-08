use std::sync::Arc;

use chrono::Utc;
use opentelemetry::global;
use sha2::Digest;
use tracing_opentelemetry::OpenTelemetrySpanExt;
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
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&req.headers));
    let span = tracing::info_span!("audio.meta_request", tap_name = %req.tap_name);
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

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

    // Request metadata from connection, with fallback to cache if unavailable
    let meta = 'fetch: {
        let Ok((connection_id, _disconnect_rx)) = tap_hub.select_connection(&tap_id).await else {
            // Connection unavailable — break and try cache fallback
            break 'fetch None;
        };
        tap_hub
            .zf_hub
            .request_audio_metadata(
                tap_id.clone(),
                connection_id,
                req.request.clone(),
                req.headers.clone(),
            )
            .await
            .ok()
    };

    let meta = match meta {
        Some(m) => m,
        None => {
            // Final cache fallback: metadata may have been populated concurrently
            // or may exist from prior audio request.
            if let Some(entry) = tap_hub.audio_cache.get_entry(&tap_id, &meta_key).await {
                tracing::info!("Metadata cache fallback hit for tap_id={}", tap_id.0);
                return Ok(AudioMetaResponse {
                    metadatas: entry.metadatas,
                    cache_key: entry.cache_key,
                    base_volume: tap.base_volume,
                });
            }
            return Err("Tap unavailable and no cached metadata found".to_string());
        }
    };

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
        let metadatas = super::wire_convert::wire_metadatas_to_domain(meta.metadatas.clone());
        let cache_key = super::wire_convert::wire_cache_policy_to_domain(meta.cache.clone());
        tokio::spawn(async move {
            if let Err(e) = cache.store_metadata(meta_item, metadatas, cache_key).await {
                tracing::warn!(%e, "Failed to store metadata in cache");
            }
        });
    }

    Ok(AudioMetaResponse {
        metadatas: super::wire_convert::wire_metadatas_to_domain(meta.metadatas),
        cache_key: super::wire_convert::wire_cache_policy_to_domain(meta.cache),
        base_volume: tap.base_volume,
    })
}
