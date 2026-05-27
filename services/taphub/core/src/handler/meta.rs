use std::sync::Arc;

use chrono::Utc;
use opentelemetry::global;
use sha2::Digest;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zakofish_taphub::ZakofishError;
use zako3_types::{
    AudioMetaResponse, AudioRequest, TapHubError,
    cache::{AudioCacheItem, AudioCacheItemKey},
};

use crate::hub::TapHub;

pub(crate) async fn handle_request_audio_meta_inner(
    tap_hub: &TapHub,
    req: AudioRequest,
) -> Result<AudioMetaResponse, TapHubError> {
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&req.headers));
    let span = tracing::info_span!("audio.meta_request", tap_id = %req.tap_id.0);
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

    let tap_id = req.tap_id.clone();

    let tap = super::tap_lookup::resolve_tap(tap_hub, &tap_id).await?;

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

    // Request metadata from connection. A connection-level miss falls back to
    // cache; a tap-script-authored failure is propagated unchanged so the user
    // sees the tap's own reason instead of the generic "unavailable" message.
    enum FetchOutcome<M> {
        Ok(M),
        ConnectionUnavailable,
        TapFailure { reason: String, try_others: bool },
    }

    let outcome = 'fetch: {
        let Ok((connection_id, _disconnect_rx)) = tap_hub.select_connection(&tap_id).await else {
            break 'fetch FetchOutcome::ConnectionUnavailable;
        };
        match tap_hub
            .zf_hub
            .request_audio_metadata(
                tap_id.clone(),
                connection_id,
                req.request.clone(),
                req.headers.clone(),
            )
            .await
        {
            Ok(m) => FetchOutcome::Ok(m),
            Err(ZakofishError::TapRequestFailure { reason, try_others }) => {
                FetchOutcome::TapFailure { reason, try_others }
            }
            Err(e) => {
                tracing::warn!(error = %e, "request_audio_metadata transport error; falling back");
                FetchOutcome::ConnectionUnavailable
            }
        }
    };

    let meta = match outcome {
        FetchOutcome::Ok(m) => m,
        FetchOutcome::TapFailure { reason, try_others } => {
            return Err(TapHubError::TapScript { reason, try_others });
        }
        FetchOutcome::ConnectionUnavailable => {
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
            return Err(TapHubError::TapUnavailable);
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
