//! The cache worker as a protofish4 sink.
//!
//! A preload used to travel tap → taphub → audio engine → here over HTTP. It
//! now comes straight from the tap over UDP, with no audio engine in the path
//! at all, which is the whole reason the cache worker gets a receiver of its
//! own.
//!
//! Two things make this safe to expose:
//!
//! * **The receiver mints.** `POST /ingest` creates the request id and key and
//!   arms the receiver *before* replying, so by the time HQ can tell a tap
//!   where to send, the slot it demultiplexes to already exists. There is no
//!   handshake and no window in which an early datagram has nowhere to go.
//! * **The key is the capability.** The datagram path needs no separate auth:
//!   the key is minted here, handed only to HQ over the admin-token RPC, and a
//!   datagram that does not authenticate under it is dropped without state
//!   change.
//!
//! Tuned for patience rather than latency — see
//! [`ReceiverConfig::cache_worker`]. Nobody is waiting on this audio, and for a
//! preload a reliable-stream abort is not a degradation but the whole request
//! failing, so a generous NACK budget buys real cache hits.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use bytes::Bytes;
use protofish4::{Endpoint, ReceiverConfig, RelOutcome, RequestId, SessionKey, Streams};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use zako3_cache_client::{CreateIngestReq, FinalizeIngestReq, IngestCreatedResp};
use zako3_preload_cache::PreloadId;

use super::preload::session::{self, PreloadError};
use crate::server::state::AppState;

/// How often the pump renews its session lease while a transfer is running.
///
/// Frames sitting behind a gap do not reach the writer, so a healthy transfer
/// in the middle of NACK recovery would otherwise look idle to the reaper. The
/// pump is the real liveness detector here — protofish4's own `idle_timeout`
/// ends a dead transfer in seconds — and the reaper is the backstop for a
/// session whose pump died, which is why this stops when the pump does.
const TOUCH_INTERVAL: Duration = Duration::from_secs(5);

/// The UDP side of the cache worker, or absent when ingest is not configured.
pub struct Ingest {
    pub endpoint: Arc<Endpoint>,
    /// Caps concurrent transfers. Each holds a reorder window, so this is the
    /// memory bound, not a politeness limit.
    pub slots: Arc<Semaphore>,
}

impl Ingest {
    pub fn new(endpoint: Arc<Endpoint>, max_sessions: usize) -> Arc<Self> {
        Arc::new(Self {
            endpoint,
            slots: Arc::new(Semaphore::new(max_sessions.max(1))),
        })
    }
}

/// Bind the socket and start its reader and timer tasks.
///
/// Called before the process reports healthy, so a bind failure is a startup
/// failure rather than a silently missing feature.
pub async fn start(bind_addr: std::net::SocketAddr, max_sessions: usize) -> anyhow::Result<Arc<Ingest>> {
    let endpoint = Endpoint::bind(bind_addr).await?;
    tracing::info!(addr = %endpoint.local_addr()?, "protofish4 ingest listening");

    tokio::spawn({
        let e = Arc::clone(&endpoint);
        async move {
            if let Err(err) = e.run().await {
                tracing::error!(%err, "protofish4 endpoint stopped");
            }
        }
    });
    // Drives per-transfer timers: NACK retries, stall detection and the
    // periodic ack that lets a sender drain its retransmission ring.
    tokio::spawn({
        let e = Arc::clone(&endpoint);
        async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(20));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                e.tick().await;
            }
        }
    });

    Ok(Ingest::new(endpoint, max_sessions))
}

/// `POST /ingest` — mint a ticket, arm the receiver, open a preload session.
///
/// The caller owns the returned session and must reach `finalize` or
/// `POST /preload/{id}/abort` on every path, including its own failures: an
/// abandoned session shadows its cache key for `GET /stream` until the reaper
/// runs.
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateIngestReq>,
) -> Result<Json<IngestCreatedResp>, StatusCode> {
    let Some(ingest) = state.ingest.clone() else {
        return Err(StatusCode::NOT_IMPLEMENTED);
    };
    let permit = Arc::clone(&ingest.slots)
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let request_id = uuid::Uuid::new_v4();
    let raw_key = protofish4::random_key();
    let key = SessionKey::from_bytes(&raw_key).map_err(|e| {
        tracing::error!(%e, "failed to build a session key");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Armed before the session exists and long before the reply is written, so
    // there is no ordering left for a caller to get wrong.
    let (armed, streams) = ingest
        .endpoint
        .arm(RequestId(request_id), key, ReceiverConfig::cache_worker())
        .await
        .map_err(|e| {
            tracing::error!(%e, %request_id, "failed to arm the ingest receiver");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let preload_id = session::open_ingest_session(&state, &req)?;

    tokio::spawn(pump(state.clone(), preload_id, request_id, armed, streams, permit));

    Ok(Json(IngestCreatedResp {
        preload_id: preload_id.0,
        request_id,
        encryption_key: raw_key,
    }))
}

/// `POST /ingest/{id}/finalize` — attach the metadata the slot was opened
/// without, releasing the session to commit.
pub async fn finalize(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(req): Json<FinalizeIngestReq>,
) -> Result<StatusCode, StatusCode> {
    session::finalize_ingest(&state, PreloadId(id), req.metadatas, req.cache_key).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Drain one transfer into its preload session and finish it.
async fn pump(
    state: AppState,
    preload_id: PreloadId,
    request_id: uuid::Uuid,
    armed: protofish4::ArmedRequest,
    mut streams: Streams,
    _permit: OwnedSemaphorePermit,
) {
    // Held for the whole transfer: dropping it disarms the receiver, and doing
    // that early would silently stop accepting the tap's packets.
    let _armed = armed;

    let Ok(session) = session::get_session(&state, preload_id) else {
        return;
    };

    let mut ticker = tokio::time::interval(TOUCH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut received = 0u64;
    let mut write_failed = false;

    loop {
        tokio::select! {
            frame = streams.rel.recv() => {
                let Some(frame) = frame else { break };
                received += 1;
                // Discard rather than stop once the disk has refused us — a
                // full volume is a normal condition here. Leaving `rel`
                // undrained would fill its channel, and the endpoint blocks on
                // that send *while holding the lock shared by every transfer on
                // the socket*, so one bad write would stall the whole worker.
                if write_failed {
                    continue;
                }
                if let Err(e) =
                    session::push_frame(&state, preload_id, Bytes::from(frame.payload)).await
                {
                    tracing::warn!(%e, %request_id, preload_id = preload_id.0, "ingest write failed");
                    write_failed = true;
                }
            }
            _ = ticker.tick() => session.touch(),
        }
    }

    let outcome = streams.outcome.await;
    let complete = matches!(outcome, Ok(RelOutcome::Complete { .. })) && !write_failed;

    if complete {
        // May not commit yet: the metadata arrives on the HTTP side, and
        // whichever half finishes last does the commit.
        if let Err(e) = session::audio_complete(&state, preload_id).await {
            tracing::warn!(%e, %request_id, preload_id = preload_id.0, "ingest commit failed");
        } else {
            tracing::info!(%request_id, preload_id = preload_id.0, frames = received, "ingest audio complete");
        }
        return;
    }

    match &outcome {
        Ok(RelOutcome::Aborted(reason)) => {
            tracing::info!(?reason, %request_id, frames = received, "ingest transfer aborted");
        }
        Err(_) => {
            tracing::info!(%request_id, frames = received, "ingest receiver closed without an outcome");
        }
        Ok(RelOutcome::Complete { .. }) => {}
    }
    // Racing the reaper here is normal and lands on the same outcome, so a
    // missing session is not a failure worth logging as one.
    match session::abort_session(&state, preload_id).await {
        Ok(()) | Err(PreloadError::NoSuchSession(_)) => {}
        Err(e) => tracing::warn!(%e, preload_id = preload_id.0, "failed to abort ingest session"),
    }
}
