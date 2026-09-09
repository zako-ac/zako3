//! Preload sessions: the transport-free lifecycle, the reaper, and the
//! interaction with the dangling-file sweep.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use zako3_cache::actions;
use zako3_cache::server::preload::{reaper, session};
use zako3_cache::server::state::{AppState, active_key};
use zako3_cache::server::WarmupState;
use zako3_cache_client::CreatePreloadReq;
use zako3_preload_cache::{AudioCache, AudioPreload, FileAudioCache};
use zako3_types::cache::{AudioCacheItem, AudioCacheItemKey};
use zako3_types::hq::TapId;
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

const TAP: &str = "tap-1";

async fn state_for(dir: &tempfile::TempDir) -> AppState {
    let cache = Arc::new(
        FileAudioCache::open_empty(dir.path().to_path_buf(), None)
            .await
            .expect("open cache"),
    );
    let preload = Arc::new(AudioPreload::new(dir.path().to_path_buf(), None));
    AppState::new(cache, preload, None, Arc::new(WarmupState::new()))
}

fn cache_item(key: &str) -> AudioCacheItem {
    AudioCacheItem {
        key: AudioCacheItemKey::CacheKey(key.to_string()),
        tap_id: TapId(TAP.to_string()),
        expire_at: None,
    }
}

fn create_req(key: &str) -> CreatePreloadReq {
    CreatePreloadReq {
        item: cache_item(key),
        metadatas: vec![AudioMetadata::Title("Song".into())],
        cache_key: AudioCachePolicy {
            cache_type: AudioCacheType::CacheKey(key.to_string()),
            ttl_seconds: None,
        },
    }
}

fn key_json(key: &str) -> String {
    serde_json::to_string(&AudioCacheItemKey::CacheKey(key.to_string())).unwrap()
}

/// Wait for the staging file to appear.
///
/// `AudioPreload` creates it from a spawned write task, so it is not there the
/// instant `open_session` returns. Note the protection in
/// `AppState::in_flight_paths` is by path rather than by existence, so the
/// guard covers this window too — there is simply nothing to delete yet.
async fn wait_for_file(path: &std::path::Path) {
    for _ in 0..200 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("staging file never appeared at {}", path.display());
}

/// Three frames of plausible Opus-shaped payload.
fn frames() -> Vec<Bytes> {
    (0..3u8).map(|i| Bytes::from(vec![i + 1; 40])).collect()
}

#[tokio::test]
async fn a_session_pushed_frame_by_frame_commits_into_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;

    let id = session::open_session(&state, create_req("k1")).expect("open");
    for f in frames() {
        session::push_frame(&state, id, f).await.expect("push");
    }
    session::finish_frames(&state, id).await.expect("finish");
    session::commit_session(&state, id).await.expect("commit");

    let entry = state
        .cache
        .get_entry(&TapId(TAP.into()), &AudioCacheItemKey::CacheKey("k1".into()))
        .await
        .expect("entry should exist after commit");
    assert!(entry.has_audio());
    assert!(!entry.is_downloading());

    // Both indexes must be clear, or a later read tails a file that is gone.
    assert!(state.sessions.is_empty());
    assert!(state.active_by_key.is_empty());
}

#[tokio::test]
async fn aborting_leaves_no_entry_and_no_staged_file() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;

    let id = session::open_session(&state, create_req("k2")).expect("open");
    session::push_frame(&state, id, frames()[0].clone()).await.unwrap();
    let staged = state.preload.frame_path(id);
    wait_for_file(&staged).await;

    session::abort_session(&state, id).await.expect("abort");

    assert!(!staged.exists(), "staged file should be gone");
    assert!(state.sessions.is_empty());
    assert!(state.active_by_key.is_empty());
    assert!(
        state
            .cache
            .get_entry(&TapId(TAP.into()), &AudioCacheItemKey::CacheKey("k2".into()))
            .await
            .is_none()
    );
}

/// The bug this whole change exists for. A producer that vanishes used to leave
/// its `active_by_key` entry behind forever, and because `GET /stream` prefers
/// an "active" preload over the committed entry, every later read of that key
/// tailed a dead partial file until the process restarted.
#[tokio::test]
async fn an_abandoned_session_is_collected_and_stops_shadowing_the_cache_key() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;

    let id = session::open_session(&state, create_req("k3")).expect("open");
    session::push_frame(&state, id, frames()[0].clone()).await.unwrap();

    let index_key = active_key(TAP, &key_json("k3"));
    assert!(state.active_by_key.contains_key(&index_key));
    let staged = state.preload.frame_path(id);
    wait_for_file(&staged).await;

    // Nothing further arrives; the producer is gone.
    tokio::time::sleep(Duration::from_millis(60)).await;
    let collected = reaper::sweep(&state, Duration::from_millis(50)).await;

    assert_eq!(collected, 1);
    assert!(state.sessions.is_empty(), "session must be dropped");
    assert!(
        !state.active_by_key.contains_key(&index_key),
        "the reverse index must be cleared, or this cache key is poisoned until restart"
    );
    assert!(!staged.exists(), "staged file must be removed");
}

/// A slow producer is not a dead one. Activity has to hold the session open, or
/// the reaper cuts off uploads that are working perfectly well.
#[tokio::test]
async fn activity_keeps_a_slow_session_alive() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;
    let ttl = Duration::from_millis(80);

    let id = session::open_session(&state, create_req("k4")).expect("open");

    for _ in 0..4 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        session::push_frame(&state, id, frames()[0].clone())
            .await
            .expect("a live producer must not be collected");
        assert_eq!(reaper::sweep(&state, ttl).await, 0);
    }

    assert!(state.sessions.contains_key(&id.0));
    session::finish_frames(&state, id).await.unwrap();
    session::commit_session(&state, id).await.expect("commit");
}

#[tokio::test]
async fn a_second_streaming_upload_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;

    let id = session::open_session(&state, create_req("k5")).expect("open");
    let _first = session::take_sender(&state, id).await.expect("first claim");

    let err = session::take_sender(&state, id).await.unwrap_err();
    assert!(
        matches!(err, session::PreloadError::AlreadyUploading(_)),
        "two concurrent uploads would interleave into one file"
    );
}

#[tokio::test]
async fn operations_on_an_unknown_session_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;
    let ghost = zako3_preload_cache::PreloadId(999);

    assert!(matches!(
        session::push_frame(&state, ghost, frames()[0].clone()).await,
        Err(session::PreloadError::NoSuchSession(_))
    ));
    assert!(matches!(
        session::commit_session(&state, ghost).await,
        Err(session::PreloadError::NoSuchSession(_))
    ));
    assert!(matches!(
        session::abort_session(&state, ghost).await,
        Err(session::PreloadError::NoSuchSession(_))
    ));
}

/// The dangling sweep deletes any `.opus` with no database row — and a preload
/// has no row until it commits. Without the guard it would delete a file that
/// is actively being written, which becomes far more likely once a preload can
/// stay open for a whole track.
#[tokio::test]
async fn the_dangling_sweep_spares_a_live_preload() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;

    let id = session::open_session(&state, create_req("k6")).expect("open");
    session::push_frame(&state, id, frames()[0].clone()).await.unwrap();
    let staged = state.preload.frame_path(id);
    wait_for_file(&staged).await;

    let protected = state.in_flight_paths();
    actions::dangling::evict_dangling(&state.cache, dir.path(), &protected)
        .await
        .expect("sweep");

    assert!(staged.exists(), "an in-flight preload must survive the sweep");

    // It still commits normally afterwards.
    session::finish_frames(&state, id).await.unwrap();
    session::commit_session(&state, id).await.expect("commit");
}

/// Once the session is gone the same file is fair game, and its lock goes too.
/// Nothing used to remove `.lock` files at all.
#[tokio::test]
async fn the_dangling_sweep_removes_a_dead_preload_and_its_lock() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_for(&dir).await;

    let id = session::open_session(&state, create_req("k7")).expect("open");
    session::push_frame(&state, id, frames()[0].clone()).await.unwrap();
    let staged = state.preload.frame_path(id);
    let lock = staged.with_extension("lock");
    wait_for_file(&staged).await;

    // Simulate a crash: the files remain, the session does not.
    state.sessions.clear();
    state.active_by_key.clear();

    let protected = state.in_flight_paths();
    assert!(protected.is_empty());
    actions::dangling::evict_dangling(&state.cache, dir.path(), &protected)
        .await
        .expect("sweep");

    assert!(!staged.exists(), "an orphaned staging file should be removed");
    assert!(!lock.exists(), "its lock should be removed too");
}
