use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};
use bytes::{BufMut, Bytes, BytesMut};
use tokio::sync::mpsc;
use zako3_cache_client::EntryQuery;
use zako3_preload_cache::{AudioCache, NextFrame, PreloadReader};
use zako3_types::{cache::AudioCacheItemKey, hq::TapId};

use super::state::{AppState, active_key};

/// `GET /stream?tap_id&key` — stream audio frames for the entry. If an active
/// preload is registered for the same `(tap_id, key)`, tail it via the signal;
/// otherwise read the committed cache file.
pub async fn stream(
    State(state): State<AppState>,
    Query(q): Query<EntryQuery>,
) -> Result<Response, StatusCode> {
    let key: AudioCacheItemKey =
        serde_json::from_str(&q.key).map_err(|_| StatusCode::BAD_REQUEST)?;
    let tap_id = TapId(q.tap_id.clone());

    // 1. Prefer tailing an active preload, so a concurrent consumer can play
    //    while the producer is still uploading.
    if let Some(preload_id) = state
        .active_by_key
        .get(&active_key(&q.tap_id, &q.key))
        .map(|r| *r)
    {
        if let Some(session) = state.sessions.get(&preload_id.into()).map(|r| r.clone()) {
            let signal = Arc::clone(&session.signal);
            if let Some(reader) = state
                .preload
                .open_reader_with_signal(session.preload_id, Arc::clone(&signal))
                .await
            {
                return Ok(body_response(spawn_reader(reader)));
            }
        }
    }

    // 2. Fall back to the committed cache file.
    let reader = state
        .cache
        .open_reader(&tap_id, &key)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(body_response(spawn_reader(reader)))
}

fn body_response(rx: mpsc::Receiver<Result<Bytes, std::io::Error>>) -> Response {
    use tokio_stream::wrappers::ReceiverStream;
    let stream = ReceiverStream::new(rx);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from_stream(stream))
        .expect("static response is well-formed")
}

/// Spawn a task that pulls frames from `reader` and pushes them re-framed
/// (`<u32 LE len><bytes>`) into an mpsc, which the HTTP body consumes.
fn spawn_reader(mut reader: PreloadReader) -> mpsc::Receiver<Result<Bytes, std::io::Error>> {
    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        loop {
            match reader.next_frame().await {
                Ok(NextFrame::Frame(frame)) => {
                    let mut out = BytesMut::with_capacity(4 + frame.len());
                    out.put_u32_le(frame.len() as u32);
                    out.extend_from_slice(&frame);
                    if tx.send(Ok(out.freeze())).await.is_err() {
                        return;
                    }
                }
                Ok(NextFrame::Pending) => continue,
                Ok(NextFrame::Done) => return,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            }
        }
    });
    rx
}
