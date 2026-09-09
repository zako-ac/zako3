//! Preload sessions, with no HTTP in sight.
//!
//! Split out from the axum handlers so the UDP ingest path can drive exactly
//! the same lifecycle. Everything here takes and returns plain values; the
//! handlers in [`super::http`] are a thin mapping onto status codes.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use bytes::Bytes;
use tokio::sync::mpsc;
use zako3_cache_client::{CreateIngestReq, CreatePreloadReq};
use zako3_preload_cache::{AudioCache, PreloadId};
use zako3_types::{AudioCachePolicy, AudioMetadata};

use crate::server::state::{AppState, PreloadSession, active_key};

/// Frames buffered between the producer and the disk writer.
pub const FRAME_CHANNEL_CAP: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum PreloadError {
    #[error("no preload session with id {0}")]
    NoSuchSession(u64),

    #[error("frames have already been uploaded for preload {0}")]
    AlreadyUploading(u64),

    #[error("the write task for preload {0} closed unexpectedly")]
    WriterGone(u64),

    #[error("staged file for preload {0} is missing")]
    StagedFileMissing(u64),

    #[error("cache key is not serialisable: {0}")]
    BadCacheKey(String),

    #[error("preload {0} was not opened as a UDP ingest")]
    NotAnIngest(u64),

    #[error("failed to store preload {id}: {source}")]
    Store {
        id: u64,
        #[source]
        source: std::io::Error,
    },
}

/// Open a session and start its disk writer.
///
/// The staging file exists from this moment, not from the first frame — which
/// is why [`AppState::in_flight_paths`] has to protect it straight away.
pub fn open_session(state: &AppState, req: CreatePreloadReq) -> Result<PreloadId, PreloadError> {
    let (preload_id, key_json, signal, sender) = stage(state, &req.item)?;
    let session = Arc::new(PreloadSession::new(
        preload_id,
        req.item.clone(),
        req.metadatas,
        req.cache_key,
        key_json.clone(),
        signal,
        sender,
    ));
    index(state, &req.item.tap_id.0, &key_json, session);
    Ok(preload_id)
}

/// Open a session for a UDP ingest, with its metadata still to come.
///
/// Identical to [`open_session`] except that it will not commit until
/// [`finalize_ingest`] supplies the metadata the tap has not sent yet.
pub fn open_ingest_session(
    state: &AppState,
    req: &CreateIngestReq,
) -> Result<PreloadId, PreloadError> {
    let (preload_id, key_json, signal, sender) = stage(state, &req.item)?;
    let session = Arc::new(PreloadSession::new_pending(
        preload_id,
        req.item.clone(),
        req.cache_key.clone(),
        key_json.clone(),
        signal,
        sender,
    ));
    index(state, &req.item.tap_id.0, &key_json, session);
    Ok(preload_id)
}

type Staged = (PreloadId, String, Arc<zako3_preload_cache::WriteSignal>, mpsc::Sender<Bytes>);

fn stage(state: &AppState, item: &zako3_types::cache::AudioCacheItem) -> Result<Staged, PreloadError> {
    let preload_id = PreloadId(uuid::Uuid::new_v4().as_u128() as u64);
    let key_json =
        serde_json::to_string(&item.key).map_err(|e| PreloadError::BadCacheKey(e.to_string()))?;
    let (sender, receiver) = mpsc::channel::<Bytes>(FRAME_CHANNEL_CAP);
    let signal = state.preload.preload(preload_id, receiver);
    Ok((preload_id, key_json, signal, sender))
}

fn index(state: &AppState, tap_id: &str, key_json: &str, session: Arc<PreloadSession>) {
    let id = session.preload_id.0;
    state.sessions.insert(id, session);
    state.active_by_key.insert(active_key(tap_id, key_json), id);
}

pub fn get_session(state: &AppState, id: PreloadId) -> Result<Arc<PreloadSession>, PreloadError> {
    state
        .sessions
        .get(&id.0)
        .map(|e| e.clone())
        .ok_or(PreloadError::NoSuchSession(id.0))
}

/// Claim exclusive use of the frame channel, for a single streaming upload.
///
/// Returns [`PreloadError::AlreadyUploading`] on a second call, which is what
/// keeps two concurrent HTTP uploads from interleaving into one file.
pub async fn take_sender(
    state: &AppState,
    id: PreloadId,
) -> Result<mpsc::Sender<Bytes>, PreloadError> {
    let session = get_session(state, id)?;
    session.touch();
    let mut guard = session.sender.lock().await;
    guard.take().ok_or(PreloadError::AlreadyUploading(id.0))
}

/// Append one Opus packet.
///
/// For producers that deliver frame by frame rather than as one stream — the
/// UDP path. Takes a bare Opus packet; the `<u32 LE len>` prefix is added by
/// the writer, so there is exactly one implementation of that framing.
pub async fn push_frame(
    state: &AppState,
    id: PreloadId,
    opus: Bytes,
) -> Result<(), PreloadError> {
    let session = get_session(state, id)?;
    session.touch();
    let sender = {
        let guard = session.sender.lock().await;
        guard.clone().ok_or(PreloadError::AlreadyUploading(id.0))?
    };
    sender
        .send(opus)
        .await
        .map_err(|_| PreloadError::WriterGone(id.0))
}

/// Signal end of input, letting the writer flush and close the file.
pub async fn finish_frames(state: &AppState, id: PreloadId) -> Result<(), PreloadError> {
    let session = get_session(state, id)?;
    session.touch();
    session.sender.lock().await.take();
    Ok(())
}

/// Move the staged file into the cache.
pub async fn commit_session(state: &AppState, id: PreloadId) -> Result<(), PreloadError> {
    let session = get_session(state, id)?;

    // Covers the case where no frames were ever uploaded: the writer is still
    // waiting on a sender that nobody dropped.
    session.sender.lock().await.take();
    wait_done(&session.signal).await;

    let opus_path = state.preload.frame_path(session.preload_id);
    if !opus_path.exists() {
        forget(state, &session);
        return Err(PreloadError::StagedFileMissing(id.0));
    }

    let target = session.target();
    let res = state
        .cache
        .store_from_path(
            session.item.clone(),
            target.metadatas,
            target.cache_key,
            &opus_path,
        )
        .await;

    forget(state, &session);

    if let Err(source) = res {
        // `store_from_path` moves the file; if it failed the staged copy is
        // still there and would otherwise linger until the next sweep.
        let _ = state.preload.delete_preload(session.preload_id).await;
        return Err(PreloadError::Store { id: id.0, source });
    }

    Ok(())
}

/// Discard the staged file without committing.
pub async fn abort_session(state: &AppState, id: PreloadId) -> Result<(), PreloadError> {
    let Some((_, session)) = state.sessions.remove(&id.0) else {
        return Err(PreloadError::NoSuchSession(id.0));
    };
    state
        .active_by_key
        .remove(&active_key(&session.item.tap_id.0, &session.key_json));

    session.sender.lock().await.take();
    wait_done(&session.signal).await;

    if let Err(e) = state.preload.delete_preload(session.preload_id).await {
        tracing::warn!(%e, preload_id = id.0, "delete_preload failed during abort");
    }
    Ok(())
}

/// Attach the metadata a UDP ingest was opened without, and commit if the
/// audio has already finished.
pub async fn finalize_ingest(
    state: &AppState,
    id: PreloadId,
    metadatas: Vec<AudioMetadata>,
    cache_key: AudioCachePolicy,
) -> Result<(), PreloadError> {
    let session = get_session(state, id)?;
    if !session.is_ingest() {
        return Err(PreloadError::NotAnIngest(id.0));
    }
    session.touch();
    session.finalize(metadatas, cache_key);
    commit_if_ready(state, id, &session).await
}

/// Record that the last audio frame is in the writer, and commit if the
/// metadata has already arrived.
pub async fn audio_complete(state: &AppState, id: PreloadId) -> Result<(), PreloadError> {
    let session = get_session(state, id)?;
    session.touch();
    finish_frames(state, id).await?;
    session.mark_audio_done();
    commit_if_ready(state, id, &session).await
}

/// Commit exactly once, from whichever of the two halves finishes last.
async fn commit_if_ready(
    state: &AppState,
    id: PreloadId,
    session: &Arc<PreloadSession>,
) -> Result<(), PreloadError> {
    if !session.committable() || !session.claim_commit() {
        return Ok(());
    }
    match commit_session(state, id).await {
        Ok(()) | Err(PreloadError::NoSuchSession(_)) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Drop a session from both indexes.
///
/// Both, always. Leaving `active_by_key` behind points every later
/// `GET /stream` for that cache key at a file that is no longer being written.
fn forget(state: &AppState, session: &PreloadSession) {
    state.sessions.remove(&session.preload_id.0);
    state
        .active_by_key
        .remove(&active_key(&session.item.tap_id.0, &session.key_json));
}

/// Block until the write task has flushed and called `signal.finish()`.
async fn wait_done(signal: &zako3_preload_cache::WriteSignal) {
    while !signal.done.load(Ordering::Acquire) {
        signal.notify.notified().await;
    }
}
