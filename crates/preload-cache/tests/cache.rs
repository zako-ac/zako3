use std::path::PathBuf;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};
use zako3_preload_cache::{AudioCache, FileAudioCache};
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetadata,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::TapId,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tap(id: &str) -> TapId {
    TapId(id.to_string())
}

fn key(s: &str) -> AudioCacheItemKey {
    AudioCacheItemKey::CacheKey(s.to_string())
}

fn policy() -> AudioCachePolicy {
    AudioCachePolicy { cache_type: AudioCacheType::None, ttl_seconds: None }
}

fn meta(title: &str) -> Vec<AudioMetadata> {
    vec![AudioMetadata::Title(title.to_string())]
}

fn item(tap_id: &str, key_str: &str) -> AudioCacheItem {
    AudioCacheItem {
        key: key(key_str),
        tap_id: tap(tap_id),
        expire_at: None,
    }
}

fn item_expiring(tap_id: &str, key_str: &str, secs_from_now: i64) -> AudioCacheItem {
    let expire_at = chrono::Utc::now() + chrono::Duration::seconds(secs_from_now);
    AudioCacheItem {
        key: key(key_str),
        tap_id: tap(tap_id),
        expire_at: Some(expire_at),
    }
}

/// Send `n` frames of `frame_bytes` bytes through `tx`, then fire the done signal.
async fn send_frames(tx: mpsc::Sender<Bytes>, done_tx: oneshot::Sender<()>, n: usize, frame_bytes: usize) {
    let payload = Bytes::from(vec![0u8; frame_bytes]);
    for _ in 0..n {
        tx.send(payload.clone()).await.unwrap();
    }
    done_tx.send(()).unwrap();
}

/// Store `n` frames into the cache and return.
async fn store_n_frames(cache: &FileAudioCache, tap_id: &str, key_str: &str, n: usize) {
    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel();
    let cache_item = item(tap_id, key_str);
    tokio::spawn(send_frames(tx, done_tx, n, 100));
    cache.store(cache_item, meta("track"), policy(), rx, done_rx).await.unwrap();
}

async fn open_cache(dir: &tempfile::TempDir) -> FileAudioCache {
    FileAudioCache::open(dir.path().to_path_buf(), None).await.unwrap()
}

// ---------------------------------------------------------------------------
// store / get_entry / open_reader
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_creates_opus_and_json_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    store_n_frames(&cache, "tap1", "k1", 3).await;

    // Both .opus and .json files should exist in the cache dir.
    let mut opus_count = 0;
    let mut json_count = 0;
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        match path.extension().and_then(|e| e.to_str()) {
            Some("opus") => opus_count += 1,
            Some("json") => json_count += 1,
            _ => {}
        }
    }
    assert_eq!(opus_count, 1, "expected 1 .opus file");
    assert_eq!(json_count, 1, "expected 1 .json sidecar");
}

#[tokio::test]
async fn get_entry_returns_stored_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    store_n_frames(&cache, "tap1", "k1", 2).await;

    let entry = cache.get_entry(&tap("tap1"), &key("k1")).await.unwrap();
    assert_eq!(
        serde_json::to_string(&entry.metadatas).unwrap(),
        serde_json::to_string(&meta("track")).unwrap()
    );
    assert!(entry.has_audio());
    assert!(!entry.is_downloading());
}

#[tokio::test]
async fn open_reader_reads_back_frames() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel();
    let frame = Bytes::from(vec![42u8; 64]);
    let frame_clone = frame.clone();
    tokio::spawn(async move {
        tx.send(frame_clone).await.unwrap();
        done_tx.send(()).unwrap();
    });
    cache.store(item("tap1", "k1"), meta("t"), policy(), rx, done_rx).await.unwrap();

    let mut reader = cache.open_reader(&tap("tap1"), &key("k1")).await.unwrap();
    let next = reader.next_frame().await.unwrap();
    match next {
        zako3_preload_cache::NextFrame::Frame(f) => assert_eq!(f, frame),
        _ => panic!("expected Frame, got Pending or Done"),
    }
}

#[tokio::test]
async fn open_reader_returns_none_for_unknown_key() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;
    assert!(cache.open_reader(&tap("tap1"), &key("missing")).await.is_none());
}

// ---------------------------------------------------------------------------
// store_from_path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_from_path_moves_file_and_creates_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    // Write a tiny opus file in a temp location.
    let src = dir.path().join("preload.opus");
    std::fs::write(&src, &[0u8, 1u8, 2u8]).unwrap();

    cache.store_from_path(item("tap1", "k1"), meta("t"), policy(), &src).await.unwrap();

    // Source file should be gone (renamed).
    assert!(!src.exists(), "source file should have been moved");

    // Entry is readable and not downloading.
    let entry = cache.get_entry(&tap("tap1"), &key("k1")).await.unwrap();
    assert!(entry.has_audio());
    assert!(!entry.is_downloading());

    // .json sidecar exists alongside the .opus in the cache dir.
    let json_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    assert_eq!(json_files.len(), 1);
}

// ---------------------------------------------------------------------------
// store_metadata
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_metadata_creates_json_no_opus() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    cache.store_metadata(item("tap1", "k1"), meta("meta-only"), policy()).await.unwrap();

    // No .opus file should exist.
    let opus_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "opus").unwrap_or(false))
        .collect();
    assert_eq!(opus_files.len(), 0, "metadata-only entry should have no .opus");

    let entry = cache.get_entry(&tap("tap1"), &key("k1")).await.unwrap();
    assert!(!entry.has_audio());
    assert_eq!(
        serde_json::to_string(&entry.metadatas).unwrap(),
        serde_json::to_string(&meta("meta-only")).unwrap()
    );
}

// ---------------------------------------------------------------------------
// delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_removes_opus_json_and_db_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    store_n_frames(&cache, "tap1", "k1", 1).await;
    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_some());

    cache.delete(&tap("tap1"), &key("k1")).await.unwrap();

    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_none());

    // No .opus or .json files should remain.
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(ext != "opus" && ext != "json", "unexpected file after delete: {path:?}");
    }
}

#[tokio::test]
async fn delete_metadata_only_removes_json() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    cache.store_metadata(item("tap1", "k1"), meta("t"), policy()).await.unwrap();
    cache.delete(&tap("tap1"), &key("k1")).await.unwrap();

    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_none());
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert_ne!(ext, "json", "json sidecar should be removed: {path:?}");
    }
}

// ---------------------------------------------------------------------------
// Expiry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_entry_returns_none_for_expired() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(send_frames(tx, done_tx, 1, 10));
    // expire 1 second in the past
    cache.store(item_expiring("tap1", "k1", -1), meta("t"), policy(), rx, done_rx).await.unwrap();

    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_none());
    assert!(cache.open_reader(&tap("tap1"), &key("k1")).await.is_none());
}

#[tokio::test]
async fn get_entry_returns_some_for_future_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(send_frames(tx, done_tx, 1, 10));
    cache.store(item_expiring("tap1", "k1", 3600), meta("t"), policy(), rx, done_rx).await.unwrap();

    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_some());
}

// ---------------------------------------------------------------------------
// Aborted stream cleanup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_cleanup_on_done_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel::<()>();
    // Send frames but drop done_tx without sending — signals incomplete stream.
    tokio::spawn(async move {
        tx.send(Bytes::from(vec![0u8; 32])).await.unwrap();
        drop(done_tx); // intentionally dropped without sending
    });
    let result = cache.store(item("tap1", "k1"), meta("t"), policy(), rx, done_rx).await;
    assert!(result.is_err());

    // DB row and files should be cleaned up.
    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_none());
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(ext != "opus" && ext != "json", "leftover file after aborted store: {path:?}");
    }
}

// ---------------------------------------------------------------------------
// max_file_bytes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_rejects_oversized_stream() {
    let dir = tempfile::tempdir().unwrap();
    // Max 10 bytes; each frame is 100 bytes.
    let cache = FileAudioCache::open(dir.path().to_path_buf(), Some(10)).await.unwrap();

    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(send_frames(tx, done_tx, 1, 100));
    let result = cache.store(item("tap1", "k1"), meta("t"), policy(), rx, done_rx).await;
    assert!(result.is_err());
    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_none());
}

// ---------------------------------------------------------------------------
// Sidecar contains full index info
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sidecar_contains_tap_id_and_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&dir).await;

    let (tx, rx) = mpsc::channel(16);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(send_frames(tx, done_tx, 1, 10));
    cache
        .store(item_expiring("tap42", "mykey", 7200), meta("t"), policy(), rx, done_rx)
        .await
        .unwrap();

    // Find the .json sidecar and parse it directly.
    let json_path = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .unwrap()
        .path();

    let raw = std::fs::read_to_string(&json_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();

    assert_eq!(v["tap_id"], "tap42");
    assert!(v["expire_at"].as_i64().unwrap() > 0, "expire_at should be a positive timestamp");
    assert!(v["created_at"].as_i64().unwrap() > 0);
    // cache_key is a serialized AudioCacheItemKey — just check it's non-empty.
    assert!(!v["cache_key"].as_str().unwrap_or("").is_empty());
}

// ---------------------------------------------------------------------------
// DB malformed recovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn malformed_db_is_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cache.db");

    // Write garbage where the DB would be.
    std::fs::write(&db_path, b"this is not a sqlite database").unwrap();

    // Opening the cache should succeed — it deletes and recreates the DB.
    let cache = FileAudioCache::open(dir.path().to_path_buf(), None).await;
    assert!(cache.is_ok(), "should recover from malformed DB");

    // The new DB should be functional.
    let cache = cache.unwrap();
    store_n_frames(&cache, "tap1", "k1", 1).await;
    assert!(cache.get_entry(&tap("tap1"), &key("k1")).await.is_some());
}

// ---------------------------------------------------------------------------
// Persistence across re-open
// ---------------------------------------------------------------------------

#[tokio::test]
async fn entry_survives_cache_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().to_path_buf();

    {
        let cache = FileAudioCache::open(path.clone(), None).await.unwrap();
        store_n_frames(&cache, "tap1", "k1", 2).await;
    }

    // Re-open the cache and the entry should still be there.
    let cache2 = FileAudioCache::open(path, None).await.unwrap();
    let entry = cache2.get_entry(&tap("tap1"), &key("k1")).await;
    assert!(entry.is_some(), "entry should survive cache re-open");
    assert_eq!(
        serde_json::to_string(&entry.unwrap().metadatas).unwrap(),
        serde_json::to_string(&meta("track")).unwrap()
    );
}
