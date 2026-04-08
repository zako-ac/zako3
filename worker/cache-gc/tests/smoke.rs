use std::path::Path;
use zako3_cache_gc::actions;
use zako3_preload_cache::FileAudioCache;
use zako3_types::cache::AudioCacheItemKey;
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

// ============================================================================
// Helpers
// ============================================================================

async fn setup_cache(dir: &tempfile::TempDir) -> FileAudioCache {
    FileAudioCache::open(dir.path().to_path_buf(), None)
        .await
        .expect("setup_cache: open FileAudioCache")
}

fn make_entry(
    tap_id: &str,
    key_suffix: &str,
    opus_path: Option<String>,
    expire_at: Option<i64>,
    use_count: i64,
    is_downloading: bool,
) -> zako3_preload_cache::db::DbEntry {
    let key = AudioCacheItemKey::CacheKey(key_suffix.to_string());
    let policy = AudioCachePolicy {
        cache_type: AudioCacheType::None,
        ttl_seconds: None,
    };

    zako3_preload_cache::db::DbEntry {
        tap_id: tap_id.to_string(),
        cache_key: serde_json::to_string(&key).expect("serialize AudioCacheItemKey"),
        opus_path,
        expire_at,
        use_count,
        last_used_at: None,
        metadatas: serde_json::to_string(&Vec::<AudioMetadata>::new())
            .expect("serialize metadatas"),
        cache_policy: serde_json::to_string(&policy).expect("serialize AudioCachePolicy"),
        created_at: chrono::Utc::now().timestamp(),
        gdsf_priority: 0.0,
        is_downloading,
    }
}

/// Write a valid `.opus` file with the custom framing format.
/// Returns the file size in bytes.
fn write_valid_opus(path: &Path, num_frames: usize) -> u64 {
    use std::io::Write;

    let mut enc = opus::Encoder::new(48000, opus::Channels::Stereo, opus::Application::Voip)
        .expect("create opus encoder");
    let pcm = vec![0i16; 960 * 2]; // silence: 20ms at 48kHz stereo

    let mut file = std::fs::File::create(path).expect("create opus file");

    for _ in 0..num_frames {
        let packet = enc
            .encode_vec(&pcm, 4000)
            .expect("encode silent frame");
        let len = packet.len() as u32;
        file.write_all(&len.to_le_bytes())
            .expect("write frame length");
        file.write_all(&packet).expect("write frame data");
    }
    file.flush().expect("flush file");

    std::fs::metadata(path).expect("get file size").len()
}

/// Write a truncated (corrupt) `.opus` file.
/// Frame length claims 50 bytes but only 10 are written.
fn write_truncated_opus(path: &Path) {
    use std::io::Write;

    let mut file = std::fs::File::create(path).expect("create truncated file");
    let fake_len: u32 = 50;
    file.write_all(&fake_len.to_le_bytes())
        .expect("write fake length");
    file.write_all(&[0u8; 10]).expect("write truncated data");
    file.flush().expect("flush file");
}

// ============================================================================
// evict_expired tests
// ============================================================================

#[tokio::test]
async fn expired_removes_past_entry_and_file() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;
    let now = chrono::Utc::now().timestamp();

    // Entry A: expired, with file
    let path_a = dir.path().join("a.opus");
    let _size_a = write_valid_opus(&path_a, 1);
    let entry_a = make_entry(
        "tap1",
        "key_a",
        Some(path_a.to_string_lossy().into_owned()),
        Some(now - 3600),
        0,
        false,
    );

    // Entry B: future expiry, with file
    let path_b = dir.path().join("b.opus");
    let _size_b = write_valid_opus(&path_b, 1);
    let entry_b = make_entry(
        "tap1",
        "key_b",
        Some(path_b.to_string_lossy().into_owned()),
        Some(now + 3600),
        0,
        false,
    );

    // Entry C: no expiry
    let entry_c = make_entry("tap1", "key_c", None, None, 0, false);

    cache.db().insert(entry_a).await.expect("insert A");
    cache.db().insert(entry_b).await.expect("insert B");
    cache.db().insert(entry_c).await.expect("insert C");

    let m = actions::expired::evict_expired(&cache)
        .await
        .expect("evict_expired");

    // A should be deleted (expired); B and C should remain
    assert_eq!(m.evict_count, 1, "should evict 1 expired entry");
    assert_eq!(m.processing_count, 2, "should process 2 entries with expire_at set");
    assert!(!path_a.exists(), "a.opus should be deleted");
    assert!(path_b.exists(), "b.opus should still exist");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 2, "2 rows should remain (B and C)");
}

#[tokio::test]
async fn expired_skips_entries_without_expire_at() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // 3 entries, all with expire_at = None
    let e1 = make_entry("tap1", "key1", None, None, 0, false);
    let e2 = make_entry("tap1", "key2", None, None, 0, false);
    let e3 = make_entry("tap1", "key3", None, None, 0, false);

    cache.db().insert(e1).await.expect("insert e1");
    cache.db().insert(e2).await.expect("insert e2");
    cache.db().insert(e3).await.expect("insert e3");

    let m = actions::expired::evict_expired(&cache)
        .await
        .expect("evict_expired");

    assert_eq!(m.evict_count, 0, "should evict 0 entries");
    assert_eq!(m.processing_count, 0, "should process 0 entries");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 3, "all 3 rows should remain");
}

// ============================================================================
// evict_dangling tests
// ============================================================================

#[tokio::test]
async fn dangling_removes_orphan_file_and_ghost_row() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // Orphan file: exists on disk, no DB row
    let path_orphan = dir.path().join("orphan.opus");
    write_valid_opus(&path_orphan, 1);

    // Known file: exists on disk, has DB row
    let path_known = dir.path().join("known.opus");
    write_valid_opus(&path_known, 1);
    let entry_known = make_entry(
        "tap1",
        "key_known",
        Some(path_known.to_string_lossy().into_owned()),
        None,
        0,
        false,
    );
    cache.db().insert(entry_known).await.expect("insert known");

    // Ghost row: DB entry exists, file does not
    let path_ghost = dir.path().join("ghost.opus");
    let entry_ghost = make_entry(
        "tap1",
        "key_ghost",
        Some(path_ghost.to_string_lossy().into_owned()),
        None,
        0,
        false,
    );
    cache.db().insert(entry_ghost).await.expect("insert ghost");

    let m = actions::dangling::evict_dangling(&cache, dir.path())
        .await
        .expect("evict_dangling");

    // Should evict orphan file + ghost row = 2 total
    assert_eq!(m.evict_count, 2, "should evict 1 orphan file + 1 ghost row");
    assert!(!path_orphan.exists(), "orphan.opus should be deleted");
    assert!(path_known.exists(), "known.opus should still exist");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 1, "only 1 row should remain (known)");
}

#[tokio::test]
async fn dangling_skips_downloading_entry() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // Non-existent path, but is_downloading = true
    let path_ghost = dir.path().join("downloading.opus");
    let entry = make_entry(
        "tap1",
        "key_downloading",
        Some(path_ghost.to_string_lossy().into_owned()),
        None,
        0,
        true, // is_downloading
    );
    cache.db().insert(entry).await.expect("insert");

    let m = actions::dangling::evict_dangling(&cache, dir.path())
        .await
        .expect("evict_dangling");

    // Downloading entries should be skipped (exempted from ghost-row eviction)
    assert_eq!(m.evict_count, 0, "should not evict downloading entry");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 1, "row should still exist");
}

// ============================================================================
// evict_gdsf tests
// ============================================================================

#[tokio::test]
async fn gdsf_evicts_lowest_priority_until_under_budget() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // File A: 1 frame, use_count=1
    let path_a = dir.path().join("a.opus");
    let size_a = write_valid_opus(&path_a, 1);
    let entry_a = make_entry(
        "tap1",
        "key_a",
        Some(path_a.to_string_lossy().into_owned()),
        None,
        1,
        false,
    );

    // File B: 1 frame, use_count=1
    let path_b = dir.path().join("b.opus");
    let size_b = write_valid_opus(&path_b, 1);
    let entry_b = make_entry(
        "tap1",
        "key_b",
        Some(path_b.to_string_lossy().into_owned()),
        None,
        1,
        false,
    );

    // File C: 20 frames (large), use_count=1 → lowest priority (1/large)
    let path_c = dir.path().join("c.opus");
    let _size_c = write_valid_opus(&path_c, 20);
    let entry_c = make_entry(
        "tap1",
        "key_c",
        Some(path_c.to_string_lossy().into_owned()),
        None,
        1,
        false,
    );

    cache.db().insert(entry_a).await.expect("insert A");
    cache.db().insert(entry_b).await.expect("insert B");
    cache.db().insert(entry_c).await.expect("insert C");

    // Set budget to exclude C (force eviction of largest file)
    let max_bytes = size_a + size_b;

    let m = actions::gdsf::evict_gdsf(&cache, max_bytes, 10)
        .await
        .expect("evict_gdsf");

    // C should be evicted (lowest priority due to size)
    assert!(m.evict_count >= 1, "should evict at least 1 entry");
    assert!(!path_c.exists(), "c.opus should be deleted (lowest priority)");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert!(
        remaining.len() < 3,
        "fewer than 3 rows should remain after eviction"
    );
}

#[tokio::test]
async fn gdsf_noop_when_under_budget() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // Single small entry
    let path = dir.path().join("small.opus");
    write_valid_opus(&path, 1);
    let entry = make_entry(
        "tap1",
        "key_small",
        Some(path.to_string_lossy().into_owned()),
        None,
        0,
        false,
    );
    cache.db().insert(entry).await.expect("insert");

    let m = actions::gdsf::evict_gdsf(&cache, u64::MAX, 10)
        .await
        .expect("evict_gdsf");

    assert_eq!(m.evict_count, 0, "should not evict when under budget");
    assert!(path.exists(), "file should still exist");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 1, "row should still exist");
}

// ============================================================================
// validate_opus tests
// ============================================================================

#[tokio::test]
async fn validate_evicts_corrupt_keeps_valid() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // Valid file
    let path_valid = dir.path().join("valid.opus");
    write_valid_opus(&path_valid, 20);
    let entry_valid = make_entry(
        "tap1",
        "key_valid",
        Some(path_valid.to_string_lossy().into_owned()),
        None,
        0,
        false,
    );

    // Corrupt (truncated) file
    let path_corrupt = dir.path().join("corrupt.opus");
    write_truncated_opus(&path_corrupt);
    let entry_corrupt = make_entry(
        "tap1",
        "key_corrupt",
        Some(path_corrupt.to_string_lossy().into_owned()),
        None,
        0,
        false,
    );

    cache
        .db()
        .insert(entry_valid)
        .await
        .expect("insert valid");
    cache
        .db()
        .insert(entry_corrupt)
        .await
        .expect("insert corrupt");

    let m = actions::validate::validate_opus(&cache)
        .await
        .expect("validate_opus");

    assert_eq!(m.processing_count, 2, "should process 2 opus files");
    assert_eq!(m.evict_count, 1, "should evict 1 corrupt file");
    assert!(!path_corrupt.exists(), "corrupt.opus should be deleted");
    assert!(path_valid.exists(), "valid.opus should still exist");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 1, "only 1 row should remain (valid)");
}

#[tokio::test]
async fn validate_skips_metadata_only_entries() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cache = setup_cache(&dir).await;

    // Metadata-only entry (opus_path = None)
    let entry = make_entry("tap1", "key_metadata", None, None, 0, false);
    cache.db().insert(entry).await.expect("insert");

    let m = actions::validate::validate_opus(&cache)
        .await
        .expect("validate_opus");

    assert_eq!(m.processing_count, 0, "should process 0 files");
    assert_eq!(m.evict_count, 0, "should evict 0 files");

    let remaining = cache.db().get_all_entries().await.expect("get remaining");
    assert_eq!(remaining.len(), 1, "row should still exist");
}
