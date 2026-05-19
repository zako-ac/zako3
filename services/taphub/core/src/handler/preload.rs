use std::sync::Arc;
use std::time::Duration;

use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_preload_cache::{AudioCache, NextFrame, PreloadId, PreloadReadEndAction};
use zako3_types::AudioMetaResponse;
use zako3_types::CachedAudioRequest;

use crate::hub::TapHub;

use super::{cache::build_cache_item, cache::resolve_metadata, stream::bridge_rel};

pub(crate) async fn handle_preload_audio_inner(
    tap_hub: &TapHub,
    req: CachedAudioRequest,
) -> Result<AudioMetaResponse, String> {
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&req.headers));
    let span = tracing::info_span!("audio.preload_request", tap_id = %req.tap_id.0);
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

    let tap_id = req.tap_id.clone();

    let tap_id_str = tap_id.0.to_string();
    let (tap_opt, conn_result) = tokio::join!(
        tap_hub.app.hq_repository.get_tap_by_id(&tap_id_str),
        tap_hub.select_connection(&tap_id),
    );
    let tap = tap_opt.ok_or_else(|| "Tap metadata not found".to_string())?;

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
    let (succ, rel, unrel) = tokio::time::timeout(
        tap_hub.request_timeout,
        tap_hub.zf_hub.request_audio(
            tap_id.clone(),
            connection_id,
            req.audio_request.clone(),
            req.headers.clone(),
        ),
    )
    .await
    .map_err(|_| format!("Tap preload timed out after {:?}", tap_hub.request_timeout))?
    .map_err(|e| format!("Failed to request audio from tap: {}", e))?;

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

    // Preload reliable stream to disk. Tap must use Dual transfer mode for
    // preload — UnreliableOnly has no persistable copy.
    let rel = rel.ok_or_else(|| {
        "Tap returned UnreliableOnly transfer for preload request".to_string()
    })?;
    let preload_id = PreloadId(uuid::Uuid::new_v4().as_u128() as u64);
    let (rel_rx, _done_rx) = bridge_rel(rel, disconnect_rx);
    let signal = tap_hub.audio_preload.preload(preload_id, rel_rx);

    // Determine finalization action
    let action = match cache_item {
        Some(item) => PreloadReadEndAction::MoveToCache {
            item,
            metadatas: metadatas.clone(),
            cache_key: succ.cache.clone(),
            cache: Arc::clone(&tap_hub.audio_cache) as Arc<dyn AudioCache>,
        },
        None => PreloadReadEndAction::Delete,
    };

    // Spawn finalization task: wait for file, drain frames, move to cache or delete
    let audio_preload = Arc::clone(&tap_hub.audio_preload);
    tokio::spawn(async move {
        // Wait for write_task to create the frame file (first notify_one).
        let ready = tokio::time::timeout(Duration::from_secs(30), signal.notify.notified()).await;
        if ready.is_err() {
            tracing::warn!(
                preload_id = preload_id.0,
                "Timed out waiting for preload file to appear"
            );
            return;
        }

        let mut reader = match audio_preload
            .open_reader_with_signal(preload_id, Arc::clone(&signal))
            .await
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    preload_id = preload_id.0,
                    "Failed to open preload reader after signal"
                );
                return;
            }
        };

        loop {
            match reader.next_frame().await {
                Ok(NextFrame::Frame(_)) => {}
                Ok(NextFrame::Pending) => {} // next_frame waited internally
                Ok(NextFrame::Done) => {
                    if let Err(e) = reader.finalize(preload_id, &audio_preload, action).await {
                        tracing::warn!(%e, "Failed to finalize preload");
                    }
                    break;
                }
                Err(e) => {
                    tracing::warn!(%e, "Preload read error during finalization");
                    break;
                }
            }
        }
    });

    Ok(AudioMetaResponse {
        metadatas,
        cache_key: succ.cache,
        base_volume: tap.base_volume,
    })
}
