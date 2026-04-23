use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use opentelemetry::{KeyValue, global};
use protofish2::Timestamp;
use sha2::Digest;
use tokio::sync::mpsc;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_preload_cache::{AudioCache, NextFrame};
use zako3_types::{AudioMetaResponse, CachedAudioRequest, hq::UserId};

use crate::hub::TapHub;
use crate::metrics;

use super::{cache::build_cache_item, cache::resolve_metadata, stream::bridge_rel};

pub(crate) async fn handle_request_audio_inner(
    tap_hub: &TapHub,
    request: CachedAudioRequest,
) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&request.headers));

    let ars = request.audio_request.to_string();

    let span = tracing::info_span!(
        "audio.request",
        tap_name = %request.tap_name.0,
        discord_user_id = %request.discord_user_id.0,
        cache_key = ?request.cache_key,
        ars = %ars,
        tap_id = tracing::field::Empty,
        cache_hit = tracing::field::Empty,
        connection_id = tracing::field::Empty,
    );
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

    let start = Instant::now();

    // Get tap ID from name
    let tap_id = tap_hub
        .state_service
        .get_tap_id_by_name(&request.tap_name)
        .await
        .map_err(|e| format!("Failed to get tap id: {}", e))?
        .ok_or_else(|| "Tap disconnected or not found".to_string())?;

    tracing::Span::current().record("tap_id", tracing::field::display(&tap_id.0));

    // Record metrics
    if let Err(e) = tap_hub.metrics_service.inc_total_uses(tap_id.clone()).await {
        tracing::warn!(%e, "Failed to increment total_uses metric");
    }

    // Fire-and-forget: notify stats subscribers via NATS
    if let Some(client) = &tap_hub.nats_client {
        let client = client.clone();
        let id = tap_id.0.to_string();
        tokio::spawn(async move {
            let payload = format!(r#"{{"tap_id":"{}"}}"#, id);
            let _ = client.publish("zako3.stats.tap_used", payload.into()).await;
        });
    }
    if let Err(e) = tap_hub
        .metrics_service
        .record_unique_user(tap_id.clone(), UserId(request.discord_user_id.0.clone()))
        .await
    {
        tracing::warn!(%e, "Failed to record unique_user metric");
    }

    let tap_id_str = tap_id.0.to_string();
    let (tap_opt, conn_result) = tokio::join!(
        tap_hub.app.hq_repository.get_tap_by_id(&tap_id_str),
        tap_hub.select_connection(&tap_id),
    );
    let tap = tap_opt.ok_or_else(|| "Tap metadata not found".to_string())?;

    super::permission::verify_permission(tap_hub, &tap, &request.discord_user_id).await?;

    // Try cache hit
    let cache_item = build_cache_item(tap_id.clone(), &request.cache_key, &request.audio_request);

    if let Some(ref item) = cache_item
        && let Some(entry) = tap_hub.audio_cache.get_entry(&item.tap_id, &item.key).await
        && entry.has_audio()
        && !entry.is_downloading()
    {
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

            metrics::metrics().cache_hits_total.add(
                1,
                &[
                    KeyValue::new("tap_id", tap_id.0.to_string()),
                    KeyValue::new("request_type", "audio"),
                ],
            );
            tracing::Span::current().record("cache_hit", true);
            tracing::info!(tap_id = %tap_id.0, cache_key = %item.key, cache_hit = true, "Cache hit");

            let (tx, rx) = mpsc::channel(100);
            tokio::spawn(async move {
                let mut reader = reader;
                let mut frame_count = 0u64;
                while let Ok(NextFrame::Frame(bytes)) = reader.next_frame().await {
                    let ts = Timestamp(frame_count * 20);
                    frame_count += 1;
                    if tx.send((ts, bytes)).await.is_err() {
                        break;
                    }
                }
            });
            let meta = AudioMetaResponse {
                metadatas: entry.metadatas,
                cache_key: entry.cache_key,
                base_volume: tap.base_volume,
            };

            let duration = start.elapsed().as_secs_f64();
            metrics::record_audio_request(&tap_id.0.to_string(), true, duration, true);

            return Ok((meta, rx));
        } else {
            tracing::warn!(
                tap_id = %tap_id.0,
                key = %item.key,
                "Cache entry found but failed to open reader"
            );
        }
    }

    tracing::Span::current().record("cache_hit", false);
    tracing::info!(
        tap_id = %tap_id.0,
        cache_key = ?cache_item.as_ref().map(|i| &i.key),
        "Cache miss — requesting from Tap"
    );

    // Cache miss: request from zakofish
    let (connection_id, disconnect_rx) = conn_result?;
    tracing::Span::current().record("connection_id", connection_id);

    let (succ, rel, mut unrel) = {
        let zakofish_span = tracing::info_span!(
            "zakofish.audio_request",
            tap_id = %tap_id.0,
            connection_id,
        );
        tokio::time::timeout(
            tap_hub.request_timeout,
            tap_hub
                .zf_hub
                .request_audio(
                    tap_id.clone(),
                    connection_id,
                    request.audio_request.clone(),
                    request.headers.clone(),
                )
                .instrument(zakofish_span),
        )
        .await
        .map_err(|_| format!("Tap request timed out after {:?}", tap_hub.request_timeout))?
        .map_err(|e| format!("Failed to request audio from tap: {}", e))?
    };

    tracing::info!(tap_id = %tap_id.0, connection_id, "Received audio from Tap");

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

        let cache_key_domain = super::wire_convert::wire_cache_policy_to_domain(cache_key);
        let cache_write_span = tracing::info_span!("cache.write", tap_id = %tap_id.0);
        tokio::spawn(
            async move {
                if let Err(e) = cache
                    .store(
                        item_clone,
                        metadatas_clone,
                        cache_key_domain,
                        rel_rx,
                        done_rx,
                    )
                    .await
                {
                    tracing::warn!(%e, "Failed to store audio in cache");
                }
            }
            .instrument(cache_write_span),
        );

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
            let cache_key2 = super::wire_convert::wire_cache_policy_to_domain(succ.cache.clone());
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
        cache_key: super::wire_convert::wire_cache_policy_to_domain(succ.cache),
        base_volume: tap.base_volume,
    };

    let duration = start.elapsed().as_secs_f64();
    metrics::record_audio_request(&tap_id.0.to_string(), false, duration, true);

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
