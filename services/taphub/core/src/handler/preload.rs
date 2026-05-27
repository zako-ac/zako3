use std::sync::Arc;

use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_types::{AudioMetaResponse, CachedAudioRequest, TapHubError};
use zakofish_taphub::ZakofishError;

use crate::hub::TapHub;

use super::{cache::build_cache_item, cache::resolve_metadata, stream::bridge_rel};

pub(crate) async fn handle_preload_audio_inner(
    tap_hub: &TapHub,
    req: CachedAudioRequest,
) -> Result<AudioMetaResponse, TapHubError> {
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&req.headers));
    let span = tracing::info_span!("audio.preload_request", tap_id = %req.tap_id.0);
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

    let tap_id = req.tap_id.clone();

    let (tap_result, conn_result) = tokio::join!(
        super::tap_lookup::resolve_tap(tap_hub, &tap_id),
        tap_hub.select_connection(&tap_id),
    );
    let tap = tap_result?;

    super::permission::verify_permission(tap_hub, &tap, &req.discord_user_id).await?;

    // Skip preload if already cached
    let cache_item = build_cache_item(tap_id.clone(), &req.cache_key, &req.audio_request);
    if let Some(ref item) = cache_item
        && let Some(entry) = tap_hub.audio_cache.get_entry(&item.tap_id, &item.key).await
        && entry.has_audio()
    {
        tracing::info!(
            "Preload cache hit for tap_id={}, key={}",
            item.tap_id.0,
            item.key
        );
        return Ok(AudioMetaResponse {
            metadatas: entry.metadatas,
            cache_key: entry.cache_key,
            base_volume: tap.base_volume,
        });
    }

    // Connection selection
    let (connection_id, disconnect_rx) = conn_result?;

    // Request audio from zakofish
    let zf_result = tokio::time::timeout(
        tap_hub.request_timeout,
        tap_hub.zf_hub.request_audio(
            tap_id.clone(),
            connection_id,
            req.audio_request.clone(),
            req.headers.clone(),
        ),
    )
    .await
    .map_err(|_| {
        TapHubError::Internal(format!(
            "Tap preload timed out after {:?}",
            tap_hub.request_timeout
        ))
    })?;

    let (succ, rel, unrel) = match zf_result {
        Ok(triple) => triple,
        Err(ZakofishError::TapRequestFailure { reason, try_others }) => {
            return Err(TapHubError::TapScript { reason, try_others });
        }
        Err(e) => {
            return Err(TapHubError::Internal(format!(
                "Failed to request audio from tap: {}",
                e
            )));
        }
    };

    // consume unrel frames to avoid buildup (but don't wait for them)
    tokio::spawn(async move {
        let mut unrel = unrel;
        while let Some(_) = unrel.recv().await {
            tokio::task::yield_now().await;
        }
    });

    // Resolve metadata
    let metadatas = resolve_metadata(
        tap_hub,
        succ.metadatas,
        &tap_id,
        &req.audio_request.to_string(),
    )
    .await;

    // Tap must use Dual transfer mode for preload — UnreliableOnly has no
    // persistable copy.
    let rel = rel.ok_or_else(|| {
        TapHubError::Internal("Tap returned UnreliableOnly transfer for preload request".to_string())
    })?;
    let (rel_rx, done_rx) = bridge_rel(rel, disconnect_rx);

    if let Some(item) = cache_item {
        // Hand the stream to the cache server. The client opens a preload
        // session, uploads frames as they arrive, and commits on done.
        let cache = Arc::clone(&tap_hub.audio_cache);
        let cache_key = succ.cache.clone();
        let metadatas_clone = metadatas.clone();
        let preload_span = tracing::info_span!("cache.preload", tap_id = %tap_id.0);
        tokio::spawn(async move {
            use tracing::Instrument;
            async move {
                if let Err(e) = cache
                    .store(item, metadatas_clone, cache_key, rel_rx, done_rx)
                    .await
                {
                    tracing::warn!(%e, "Failed to preload audio into cache");
                }
            }
            .instrument(preload_span)
            .await
        });
    } else {
        // No cache policy — drain rel_rx so the bridge task can exit cleanly.
        tokio::spawn(async move {
            let mut rel_rx = rel_rx;
            while rel_rx.recv().await.is_some() {}
            let _ = done_rx.await;
        });
    }

    Ok(AudioMetaResponse {
        metadatas,
        cache_key: succ.cache,
        base_volume: tap.base_volume,
    })
}
