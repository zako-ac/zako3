//! HTTP handlers over [`super::core`]. Nothing but transport mapping lives here.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use bytes::{Buf, BytesMut};
use futures_util::StreamExt;
use zako3_cache_client::{CreatePreloadReq, PreloadCreatedResp};
use zako3_preload_cache::PreloadId;

use super::session::{self, PreloadError};
use crate::server::state::AppState;

impl From<PreloadError> for StatusCode {
    fn from(e: PreloadError) -> Self {
        match e {
            PreloadError::NoSuchSession(_) => StatusCode::NOT_FOUND,
            PreloadError::AlreadyUploading(_) => StatusCode::CONFLICT,
            PreloadError::BadCacheKey(_) => StatusCode::BAD_REQUEST,
            PreloadError::WriterGone(_)
            | PreloadError::StagedFileMissing(_)
            | PreloadError::Store { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// `POST /preload` — open a new preload session bound to a cache target.
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreatePreloadReq>,
) -> Result<Json<PreloadCreatedResp>, StatusCode> {
    let preload_id = session::open_session(&state, req)?;
    Ok(Json(PreloadCreatedResp { preload_id: preload_id.0 }))
}

/// `POST /preload/{id}/frames` — stream framed bytes into the preload writer.
/// Body framing is `<u32 LE len><bytes>` repeated, matching the on-disk format.
pub async fn frames(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    body: axum::body::Body,
) -> Result<StatusCode, StatusCode> {
    let id = PreloadId(id);
    let sender = session::take_sender(&state, id).await?;
    let live = session::get_session(&state, id)?;

    let mut data = body.into_data_stream();
    let mut buf = BytesMut::new();
    while let Some(chunk) = data.next().await {
        let chunk = chunk.map_err(|e| {
            tracing::warn!(%e, "preload frames body read error");
            StatusCode::BAD_REQUEST
        })?;
        // A long upload must keep the session looking alive, or the reaper will
        // collect it out from under a producer that is working perfectly well.
        live.touch();
        buf.extend_from_slice(&chunk);
        while let Some(frame) = next_frame(&mut buf) {
            if sender.send(frame).await.is_err() {
                tracing::warn!(preload_id = id.0, "write task closed unexpectedly");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // Dropping the sender signals the write task to flush and finish.
    drop(sender);
    Ok(StatusCode::OK)
}

/// Split one `<u32 LE len><bytes>` frame off the front of `buf`, if a whole one
/// is there yet.
fn next_frame(buf: &mut BytesMut) -> Option<bytes::Bytes> {
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + len {
        return None;
    }
    buf.advance(4);
    Some(buf.split_to(len).freeze())
}

/// `POST /preload/{id}/commit` — wait for the write task to finish, then move
/// the staged file into the cache.
pub async fn commit(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, StatusCode> {
    session::commit_session(&state, PreloadId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /preload/{id}/abort` — drop the staged file without committing.
pub async fn abort(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, StatusCode> {
    session::abort_session(&state, PreloadId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
