use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use bytes::{Buf, Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::sync::{Mutex, mpsc};
use zako3_cache_client::{CreatePreloadReq, PreloadCreatedResp};
use zako3_preload_cache::{AudioCache, PreloadId};

use super::state::{AppState, PreloadSession, active_key};

const FRAME_CHANNEL_CAP: usize = 100;

/// `POST /preload` — open a new preload session bound to a cache target.
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreatePreloadReq>,
) -> Result<Json<PreloadCreatedResp>, StatusCode> {
    let preload_id = PreloadId(uuid::Uuid::new_v4().as_u128() as u64);
    let key_json = serde_json::to_string(&req.item.key)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let (sender, receiver) = mpsc::channel::<Bytes>(FRAME_CHANNEL_CAP);
    let signal = state.preload.preload(preload_id, receiver);

    let session = Arc::new(PreloadSession {
        preload_id,
        item: req.item.clone(),
        metadatas: req.metadatas,
        cache_key: req.cache_key,
        key_json: key_json.clone(),
        signal,
        sender: Mutex::new(Some(sender)),
    });

    state.sessions.insert(preload_id.0, session);
    state
        .active_by_key
        .insert(active_key(&req.item.tap_id.0, &key_json), preload_id.0);

    Ok(Json(PreloadCreatedResp {
        preload_id: preload_id.0,
    }))
}

/// `POST /preload/{id}/frames` — stream framed bytes into the preload writer.
/// Body framing is `<u32 LE len><bytes>` repeated, matching the on-disk format.
pub async fn frames(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    body: axum::body::Body,
) -> Result<StatusCode, StatusCode> {
    let session = state
        .sessions
        .get(&id)
        .ok_or(StatusCode::NOT_FOUND)?
        .clone();

    let mut sender_guard = session.sender.lock().await;
    let sender = sender_guard.take().ok_or(StatusCode::CONFLICT)?;
    drop(sender_guard);

    let mut data = body.into_data_stream();
    let mut buf = BytesMut::new();
    while let Some(chunk) = data.next().await {
        let chunk = chunk.map_err(|e| {
            tracing::warn!(%e, "preload frames body read error");
            StatusCode::BAD_REQUEST
        })?;
        buf.extend_from_slice(&chunk);
        loop {
            if buf.len() < 4 {
                break;
            }
            let frame_len =
                u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            if buf.len() < 4 + frame_len {
                break;
            }
            buf.advance(4);
            let frame = buf.split_to(frame_len).freeze();
            if sender.send(frame).await.is_err() {
                tracing::warn!(preload_id = id, "write task closed unexpectedly");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // Dropping the sender signals the write task to flush and finish.
    drop(sender);
    Ok(StatusCode::OK)
}

/// `POST /preload/{id}/commit` — wait for the write task to finish, then move
/// the staged file into the cache.
pub async fn commit(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, StatusCode> {
    let session = {
        let entry = state.sessions.get(&id).ok_or(StatusCode::NOT_FOUND)?;
        entry.clone()
    };

    // If /frames was never called, drop the sender now so the write task exits.
    {
        let mut guard = session.sender.lock().await;
        guard.take();
    }

    wait_done(&session.signal).await;

    let opus_path = state.preload.frame_path(session.preload_id);
    if !opus_path.exists() {
        cleanup(&state, &session);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let res = state
        .cache
        .store_from_path(
            session.item.clone(),
            session.metadatas.clone(),
            session.cache_key.clone(),
            &opus_path,
        )
        .await;

    cleanup(&state, &session);

    // store_from_path moves the file; if it failed, best-effort remove the temp.
    if let Err(e) = res {
        tracing::warn!(%e, preload_id = id, "store_from_path failed during commit");
        let _ = state.preload.delete_preload(session.preload_id).await;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `POST /preload/{id}/abort` — drop the staged file without committing.
pub async fn abort(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, StatusCode> {
    let Some((_, session)) = state.sessions.remove(&id) else {
        return Err(StatusCode::NOT_FOUND);
    };
    state
        .active_by_key
        .remove(&active_key(&session.item.tap_id.0, &session.key_json));

    {
        let mut guard = session.sender.lock().await;
        guard.take();
    }
    wait_done(&session.signal).await;

    if let Err(e) = state.preload.delete_preload(session.preload_id).await {
        tracing::warn!(%e, preload_id = id, "delete_preload failed during abort");
    }
    Ok(StatusCode::NO_CONTENT)
}

fn cleanup(state: &AppState, session: &PreloadSession) {
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
