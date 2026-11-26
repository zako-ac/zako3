use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use opentelemetry::{KeyValue, global};
use sha2::Digest;
use tokio::sync::mpsc;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_audio_probe::{Stage, UpstreamHopWriter, dropping, hop_record_key, hop_record_ttl_secs};
use zako3_preload_cache::NextFrame;
use zako3_states::CacheRepositoryRef;
use zako3_taphub_transport_server::Timestamp;
use zako3_types::{AudioMetaResponse, CachedAudioRequest, TapHubError};
use zakofish_taphub::ZakofishError;

use crate::hub::TapHub;
use crate::metrics;

use super::cancel::{Waited, wait_for_tap};
use super::{cache::build_cache_item, cache::resolve_metadata, stream::bridge_rel};

/// Connections one audio request may be sent to.
const MAX_TAP_ATTEMPTS: usize = 2;

/// A retry is only started if the request budget can still hold one. Below
/// this there is no point handing the request to a second tap: the caller has
/// already given up by the time the answer could arrive.
const MIN_RETRY_BUDGET: Duration = Duration::from_millis(1500);

pub(crate) async fn handle_request_audio_inner(
    tap_hub: &TapHub,
    request: CachedAudioRequest,
) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), TapHubError> {
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&request.headers));

    let ars = request.audio_request.to_string();
    // The id both ends derive from the request, so the engine can find this
    // stream's hop record by key without either side telling the other.
    let stream_id = zako3_audio_probe::request_stream_id(
        &request.tap_id.0.to_string(),
        &request.discord_user_id.0.to_string(),
        &ars,
    );

    let span = tracing::info_span!(
        "audio.request",
        tap_id = %request.tap_id.0,
        discord_user_id = %request.discord_user_id.0,
        cache_key = ?request.cache_key,
        ars = %ars,
        cache_hit = tracing::field::Empty,
        connection_id = tracing::field::Empty,
    );
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

    let start = Instant::now();
    // The budget covers the whole request, not just the tap leg: the tap lookup,
    // the permission check and the cache query below all come out of it.
    let deadline = start + tap_hub.request_timeout;

    let tap_id = request.tap_id.clone();

    let trace_id = {
        use opentelemetry::trace::TraceContextExt;
        let id = tracing::Span::current()
            .context()
            .span()
            .span_context()
            .trace_id()
            .to_string();
        if id == "00000000000000000000000000000000" {
            None
        } else {
            Some(id)
        }
    };

    let (tap_result, conn_result) = tokio::join!(
        super::tap_lookup::resolve_tap(tap_hub, &tap_id),
        tap_hub.select_connection(&tap_id),
    );
    let tap = tap_result?;

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
            let writer = Arc::new(UpstreamHopWriter::new(
                stream_id.clone(),
                tap_id.0.to_string(),
                "cache",
            ));
            let hop_repo = tap_hub.app.cache_repository.clone();
            tokio::spawn(async move {
                let mut reader = reader;
                let mut frame_count = 0u64;
                while let Ok(NextFrame::Frame(bytes)) = reader.next_frame().await {
                    let ts = Timestamp(frame_count * 20);
                    frame_count += 1;
                    writer.on_source_frame();
                    writer.on_send();
                    publish_hops(&hop_repo, &writer);
                    // The injection models a loss *after* taphub sent: the
                    // record keeps advancing, the frame never reaches the
                    // engine. Dropping before the bookkeeping would instead look
                    // like taphub never sent, which is a different fault.
                    if dropping(writer.stream(), Stage::TaphubSend) {
                        zako3_audio_probe::record_injected_drop(Stage::TaphubSend, 1);
                        continue;
                    }
                    if tx.send((ts, bytes)).await.is_err() {
                        break;
                    }
                }
                // The stream is over; drop the record so a later stall cannot
                // read this stream's frozen send time as a live hop.
                clear_hops(&hop_repo, &writer);
            });
            let meta = AudioMetaResponse {
                metadatas: entry.metadatas,
                cache_key: entry.cache_key,
                base_volume: tap.base_volume,
            };

            let duration = start.elapsed().as_secs_f64();
            metrics::record_audio_request(&tap_id.0.to_string(), true, duration, true);

            {
                let entry = zako3_types::hq::history::UseHistoryEntry::PlayAudio(
                    zako3_types::hq::history::PlayAudioHistory {
                        user_id: None,
                        discord_user_id: Some(request.discord_user_id.clone()),
                        ars_length: ars.len(),
                        trace_id: trace_id.clone(),
                        tap_id: tap_id.clone(),
                        cache_hit: true,
                        success: true,
                    },
                );
                let pubsub = tap_hub.history_pubsub.clone();
                let metrics = tap_hub.metrics_service.clone();
                let metrics_tap_id = tap_id.clone();
                tokio::spawn(async move {
                    let _ = metrics.incr_delta_total_uses(&metrics_tap_id).await;
                    let _ = metrics.incr_delta_cache_hits(&metrics_tap_id).await;
                    if let Err(e) = pubsub.publish_history(&entry).await {
                        tracing::warn!(%e, "Failed to publish history (cache hit)");
                    }
                });
            }

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

    // Cache miss: request from zakofish.
    //
    // Two attempts at most, on two different connections. A tap connection can
    // be dead while the transport still believes it is alive — the pod is gone
    // but the socket has not timed out yet — and nothing reports that, so the
    // first request to notice it sits out the whole budget. The load test hit
    // exactly this after a scale-down: the surviving replicas were healthy and
    // every request still went to a connection whose peer was gone, for eight
    // minutes. So a connection that does not answer is recorded as such (three
    // in a row and it leaves the routing set, see `TapHub::mark_unanswered`),
    // and if the budget still has room the request is handed to another
    // connection instead of failing.
    let mut tried: Vec<u64> = Vec::new();
    let mut next_conn = Some(conn_result?);
    let mut final_error = TapHubError::TapUnavailable;

    let (connection_id, succ, rel, mut unrel, disconnect_rx) = loop {
        let Some((connection_id, mut disconnect_rx)) = next_conn.take() else {
            // No connection left to try: the tap is gone or every replica is
            // suspect. `TapUnavailable` is what `select_connection` reports for
            // that, and it is what the caller retries.
            return Err(final_error);
        };
        tried.push(connection_id);
        tracing::Span::current().record("connection_id", connection_id);

        let zakofish_span = tracing::info_span!(
            "zakofish.audio_request",
            tap_id = %tap_id.0,
            connection_id,
        );
        // One chan per attempt, counted where it is opened: opened minus closed
        // is the number of chans this connection still has open on the wire.
        tap_hub.record_stream_opened(&tap_id, connection_id);
        let request_fut = tap_hub
            .zf_hub
            .request_audio(
                tap_id.clone(),
                connection_id,
                request.audio_request.clone(),
                request.headers.clone(),
            )
            .instrument(zakofish_span);

        // Bounded by what is left of the request budget and by the tap
        // connection's life; either one ending drops the request, which closes
        // only this request's chan.
        final_error = match wait_for_tap(request_fut, deadline, &mut disconnect_rx).await {
            Waited::Done(Ok(triple)) => {
                tap_hub.mark_answered(connection_id);
                tap_hub.record_stream_closed(&tap_id, connection_id);
                let (succ, rel, unrel) = triple;
                break (connection_id, succ, rel, unrel, disconnect_rx);
            }
            Waited::Done(Err(ZakofishError::TapRequestFailure { reason, try_others })) => {
                // The tap answered, and its answer was "no". Asking another
                // replica the same question would only get the same answer. The
                // chan completed, so it is not an open one.
                tap_hub.record_stream_closed(&tap_id, connection_id);
                return Err(TapHubError::TapScript { reason, try_others });
            }
            Waited::Done(Err(e)) => {
                tap_hub.mark_unanswered(&tap_id, connection_id);
                TapHubError::Internal(format!("Failed to request audio from tap: {}", e))
            }
            Waited::TimedOut => {
                tap_hub.mark_unanswered(&tap_id, connection_id);
                TapHubError::Internal(format!(
                    "Tap request timed out after {:?}",
                    tap_hub.request_timeout
                ))
            }
            Waited::Disconnected => {
                // The tap is gone, which is the same condition
                // `select_connection` reports as unavailable — not an internal
                // fault.
                tap_hub.mark_unanswered(&tap_id, connection_id);
                TapHubError::TapUnavailable
            }
        };

        // A retry is only started when the budget can still hold one. A request
        // that already spent the whole budget has nothing left to try with —
        // that is the case `mark_unanswered` exists for.
        let remaining = deadline.saturating_duration_since(Instant::now());
        if tried.len() >= MAX_TAP_ATTEMPTS || remaining < MIN_RETRY_BUDGET {
            return Err(final_error);
        }

        match tap_hub.select_connection_excluding(&tap_id, &tried).await {
            Ok(conn) => {
                tracing::warn!(
                    tap_id = %tap_id.0,
                    connection_id,
                    remaining_ms = remaining.as_millis() as u64,
                    "tap connection did not answer — retrying on another connection"
                );
                next_conn = Some(conn);
            }
            Err(e) => return Err(e),
        }
    };

    tracing::info!(tap_id = %tap_id.0, connection_id, "Received audio from Tap");

    // Bridge reliable stream (only present in Dual transfer mode; UnreliableOnly
    // skips caching since there is no authoritative copy to persist).
    let rel_bridge = rel.map(|r| bridge_rel(r, disconnect_rx));

    // Resolve metadata
    let metadatas = resolve_metadata(
        tap_hub,
        succ.metadatas,
        &tap_id,
        &request.audio_request.to_string(),
    )
    .await;

    if let (Some(item), Some((rel_rx, done_rx))) = (cache_item, rel_bridge) {
        let cache = Arc::clone(&tap_hub.audio_cache);
        let metadatas_clone = metadatas.clone();
        let cache_key = succ.cache.clone();
        let item_clone = item.clone();

        let cache_key_domain = cache_key;
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

    let duration = start.elapsed().as_secs_f64();
    metrics::record_audio_request(&tap_id.0.to_string(), false, duration, true);

    let (tx, rx) = mpsc::channel(100);
    let writer = Arc::new(UpstreamHopWriter::new(
        stream_id.clone(),
        tap_id.0.to_string(),
        "tap",
    ));
    let hop_repo = tap_hub.app.cache_repository.clone();
    tokio::spawn(async move {
        // `unrel` (zakofish) yields zakofish's `Timestamp`; re-wrap it in the
        // transport's own `Timestamp` for the pf3 transfer.
        while let Some((ts, bytes)) = unrel.recv().await {
            writer.on_source_frame();
            writer.on_send();
            publish_hops(&hop_repo, &writer);
            // See the cache path: the injection drops after the send is
            // recorded, so the record honestly says taphub was still sending.
            if dropping(writer.stream(), Stage::TaphubSend) {
                zako3_audio_probe::record_injected_drop(Stage::TaphubSend, 1);
                continue;
            }
            if let Err(e) = tx.send((Timestamp(ts.0), bytes)).await {
                tracing::warn!(%e, "Failed to send audio chunk to channel");
                break;
            }
        }
        // The stream is over; drop the record so a later stall cannot read this
        // stream's frozen send time as a live hop.
        clear_hops(&hop_repo, &writer);
    });

    {
        let entry = zako3_types::hq::history::UseHistoryEntry::PlayAudio(
            zako3_types::hq::history::PlayAudioHistory {
                user_id: None,
                discord_user_id: Some(request.discord_user_id.clone()),
                ars_length: ars.len(),
                trace_id: trace_id.clone(),
                tap_id: tap_id.clone(),
                cache_hit: false,
                success: true,
            },
        );
        let pubsub = tap_hub.history_pubsub.clone();
        let metrics = tap_hub.metrics_service.clone();
        let metrics_tap_id = tap_id.clone();
        tokio::spawn(async move {
            let _ = metrics.incr_delta_total_uses(&metrics_tap_id).await;
            if let Err(e) = pubsub.publish_history(&entry).await {
                tracing::warn!(%e, "Failed to publish history (cache miss)");
            }
        });
    }

    Ok((meta, rx))
}

/// Republish this stream's hop record, at most once per publish interval.
///
/// Written off the frame path: the record is a diagnostic, and a slow Redis must
/// never be what makes an audio stream late. The TTL is short, so a stream that
/// ends simply stops being visible to a reader rather than leaving a stale
/// record that looks like a live-but-quiet hop.
fn clear_hops(repo: &CacheRepositoryRef, writer: &UpstreamHopWriter) {
    let key = hop_record_key(writer.stream());
    let repo = Arc::clone(repo);
    tokio::spawn(async move {
        repo.del(&key).await;
    });
}

fn publish_hops(repo: &CacheRepositoryRef, writer: &UpstreamHopWriter) {
    if !writer.should_publish() {
        return;
    }
    let key = hop_record_key(writer.stream());
    let Ok(json) = serde_json::to_string(&writer.snapshot()) else {
        return;
    };
    let repo = Arc::clone(repo);
    tokio::spawn(async move {
        repo.set_ex(&key, &json, hop_record_ttl_secs()).await;
    });
}
