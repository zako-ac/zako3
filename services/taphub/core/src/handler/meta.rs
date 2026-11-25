use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use opentelemetry::global;
use sha2::Digest;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_types::{
    AudioMetaResponse, AudioRequest, TapHubError,
    cache::{AudioCacheItem, AudioCacheItemKey},
};
use zakofish_taphub::ZakofishError;

use super::cancel::Waited;
use crate::hub::TapHub;

/// Connections one metadata request may be sent to.
///
/// Two is enough to step over a single wedged replica, and the request budget
/// rarely holds a third attempt.
const MAX_META_ATTEMPTS: usize = 2;

/// The smallest half-budget a connection on probation is given before the
/// request is tried on another connection.
const MIN_PROBATION_BUDGET: Duration = Duration::from_millis(750);

/// The smallest remainder worth starting a retry with: below this the caller
/// has given up before a second tap could answer.
const MIN_META_RETRY_BUDGET: Duration = Duration::from_millis(750);

pub(crate) async fn handle_request_audio_meta_inner(
    tap_hub: &TapHub,
    req: AudioRequest,
) -> Result<AudioMetaResponse, TapHubError> {
    let parent_cx = global::get_text_map_propagator(|p| p.extract(&req.headers));
    let span = tracing::info_span!("audio.meta_request", tap_id = %req.tap_id.0);
    let _ = span.set_parent(parent_cx);
    let _enter = span.enter();

    let tap_id = req.tap_id.clone();
    // The budget belongs to the whole request, not to the tap leg alone: the tap
    // lookup, the permission check and the cache query above can each eat into
    // it, and a request the caller has already given up on is not worth the
    // remainder.
    let deadline = Instant::now() + tap_hub.request_timeout;

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
        Unavailable(Unavailable),
        TapFailure { reason: String, try_others: bool },
    }

    /// Why there is no answer from the tap. Kept apart because they mean
    /// different things to the caller: no connection is `TapUnavailable`, but a
    /// tap that is connected and simply too slow is not "offline".
    enum Unavailable {
        NoConnection,
        TimedOut,
        Disconnected,
        Transport,
    }

    // The metadata request is bounded twice: by what is left of the request
    // budget, and by the life of the tap connection it is made on. Cancelling it
    // drops the request's chan, and only that chan: the tap connection is a clone
    // held by the hub, and nothing on this path closes it.
    //
    // A connection that has already been retired for not answering gets half the
    // remaining budget instead of all of it, and the other half buys a second
    // attempt on a different connection. That split is only for connections on
    // probation: a healthy tap keeps the whole budget, because metadata is not
    // always instant (the YouTube tap resolves a URL in this handler) and
    // shortening its leash would break taps that are merely slow.
    //
    // This is the difference between "some messages are read and some are not"
    // and a working tap: with two replicas and one wedged connection, a request
    // routed to the wedged one used to fail outright while its neighbour was
    // healthy, so roughly every other message was dropped (2026-11-26).
    let mut tried: Vec<u64> = Vec::new();
    let mut next_conn = tap_hub.select_connection(&tap_id).await.ok();
    let mut last_kind = Unavailable::NoConnection;

    let outcome = loop {
        let Some((connection_id, mut disconnect_rx)) = next_conn.take() else {
            // Nothing left to try: no connection, or every replica already
            // failed this request.
            break FetchOutcome::Unavailable(last_kind);
        };
        tried.push(connection_id);

        let now = Instant::now();
        let remaining = deadline.saturating_duration_since(now);
        let on_probation = tap_hub.is_on_probation(connection_id);
        let attempt_deadline = if on_probation {
            std::cmp::min(deadline, now + (remaining / 2).max(MIN_PROBATION_BUDGET))
        } else {
            deadline
        };

        let request = tap_hub.zf_hub.request_audio_metadata(
            tap_id.clone(),
            connection_id,
            req.request.clone(),
            req.headers.clone(),
        );
        let kind = match super::cancel::wait_for_tap(request, attempt_deadline, &mut disconnect_rx)
            .await
        {
            Waited::Done(Ok(m)) => {
                tap_hub.mark_answered(connection_id);
                break FetchOutcome::Ok(m);
            }
            Waited::Done(Err(ZakofishError::TapRequestFailure { reason, try_others })) => {
                // The tap answered with a refusal of its own; that is not a
                // connection problem and must not count against it.
                tap_hub.mark_answered(connection_id);
                break FetchOutcome::TapFailure { reason, try_others };
            }
            Waited::Done(Err(e)) => {
                tap_hub.mark_unanswered(&tap_id, connection_id);
                tracing::warn!(error = %e, "request_audio_metadata transport error; falling back");
                Unavailable::Transport
            }
            Waited::TimedOut => {
                tap_hub.mark_unanswered(&tap_id, connection_id);
                tracing::warn!(
                    timeout_ms = attempt_deadline.saturating_duration_since(now).as_millis() as u64,
                    "request_audio_metadata timed out; falling back"
                );
                Unavailable::TimedOut
            }
            Waited::Disconnected => {
                tap_hub.mark_unanswered(&tap_id, connection_id);
                tracing::warn!("tap disconnected during the metadata request; falling back");
                Unavailable::Disconnected
            }
        };
        last_kind = kind;

        // Only a probation attempt is short enough to leave the budget for
        // another connection; anything else has spent what it was given.
        let remaining = deadline.saturating_duration_since(Instant::now());
        if !on_probation || tried.len() >= MAX_META_ATTEMPTS || remaining < MIN_META_RETRY_BUDGET {
            break FetchOutcome::Unavailable(last_kind);
        }

        match tap_hub.select_connection_excluding(&tap_id, &tried).await {
            Ok(conn) => {
                tracing::warn!(
                    tap_id = %tap_id.0,
                    connection_id,
                    remaining_ms = remaining.as_millis() as u64,
                    "tap connection on probation did not answer the metadata request — retrying on another connection"
                );
                next_conn = Some(conn);
            }
            Err(_) => break FetchOutcome::Unavailable(last_kind),
        }
    };

    let meta = match outcome {
        FetchOutcome::Ok(m) => m,
        FetchOutcome::TapFailure { reason, try_others } => {
            return Err(TapHubError::TapScript { reason, try_others });
        }
        FetchOutcome::Unavailable(kind) => {
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
            return Err(match kind {
                Unavailable::NoConnection | Unavailable::Disconnected => {
                    TapHubError::TapUnavailable
                }
                Unavailable::TimedOut => TapHubError::Internal(format!(
                    "Tap metadata request timed out after {:?}",
                    tap_hub.request_timeout
                )),
                Unavailable::Transport => {
                    TapHubError::Internal("Tap metadata request failed in transport".to_string())
                }
            });
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
