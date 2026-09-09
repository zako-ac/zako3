//! The cache tee's isolation guarantee.
//!
//! The property: playback may feed the cache, but the cache can never make
//! playback wait. These assert that structurally — that `offer` returns
//! immediately no matter what the cache server is doing, and that a tee which
//! cannot run degrades to "no caching" rather than to an error.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use zako3_audio_engine_infra::cache_tee::{CacheTee, CacheTeeFactory, TeeDisabled};
use zako3_cache_client::RemoteAudioCache;
use zako3_types::cache::{AudioCacheItem, AudioCacheItemKey};
use zako3_types::hq::TapId;
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

/// Points at a port nothing is listening on, so every upload fails the way a
/// dead cache server would.
fn dead_cache() -> Arc<RemoteAudioCache> {
    Arc::new(RemoteAudioCache::new("http://127.0.0.1:1".to_string(), None).expect("client"))
}

fn item() -> AudioCacheItem {
    AudioCacheItem {
        key: AudioCacheItemKey::CacheKey("k".into()),
        tap_id: TapId("tap-1".into()),
        expire_at: None,
    }
}

fn policy() -> AudioCachePolicy {
    AudioCachePolicy {
        cache_type: AudioCacheType::CacheKey("k".into()),
        ttl_seconds: None,
    }
}

fn metas() -> Vec<AudioMetadata> {
    vec![AudioMetadata::Title("Song".into())]
}

#[tokio::test]
async fn an_uncacheable_request_gets_no_tee() {
    let factory = CacheTeeFactory::new(dead_cache());
    let tee = factory.open(None, metas(), policy());

    assert!(!tee.is_active());
    assert!(matches!(
        tee,
        CacheTee::Disabled(TeeDisabled::NotCacheable)
    ));
}

/// Offering to a disabled tee has to be a no-op rather than an error, so the
/// playback path never branches on whether caching is happening.
#[tokio::test]
async fn offering_to_a_disabled_tee_is_harmless() {
    let factory = CacheTeeFactory::new(dead_cache());
    let tee = factory.open(None, metas(), policy());

    for _ in 0..1000 {
        tee.offer(Bytes::from_static(b"frame"));
    }
    tee.commit();
}

/// The core guarantee. With nothing listening on the other end, a thousand
/// frames must still be accepted without blocking — the queue fills, frames are
/// dropped, and playback never notices.
#[tokio::test]
async fn a_dead_cache_server_never_blocks_the_caller() {
    let factory = CacheTeeFactory::new(dead_cache());
    let tee = factory.open(Some(item()), metas(), policy());
    assert!(tee.is_active());

    let started = Instant::now();
    for _ in 0..5_000 {
        tee.offer(Bytes::from(vec![0u8; 200]));
    }
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "offering must not wait on the cache server; took {elapsed:?}"
    );
}

/// Dropping a tee without committing must abort. A truncated entry is worse
/// than no entry, because it would be served as if it were whole.
#[tokio::test]
async fn dropping_a_tee_without_committing_does_not_commit() {
    let factory = CacheTeeFactory::new(dead_cache());
    let tee = factory.open(Some(item()), metas(), policy());

    tee.offer(Bytes::from_static(b"partial"));
    drop(tee);

    // The upload task notices and records an abort rather than a commit.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        factory.aborted_count() > 0,
        "an abandoned tee should be counted as an abort"
    );
}

/// A cache server that is down must cost a bounded amount, not a connect
/// timeout per request for as long as the outage lasts.
#[tokio::test]
async fn repeated_failures_open_the_breaker() {
    let factory = CacheTeeFactory::new(dead_cache());

    // Drive enough failures to trip it.
    for _ in 0..8 {
        let tee = factory.open(Some(item()), metas(), policy());
        tee.offer(Bytes::from_static(b"x"));
        drop(tee);
        tokio::time::sleep(Duration::from_millis(60)).await;
    }

    let tee = factory.open(Some(item()), metas(), policy());
    assert!(
        matches!(tee, CacheTee::Disabled(TeeDisabled::CircuitOpen)),
        "the breaker should stop further attempts while the cache is down"
    );
}

/// Concurrency is bounded so a burst of requests cannot open unbounded uploads.
#[tokio::test]
async fn concurrent_uploads_are_capped() {
    let factory = CacheTeeFactory::new(dead_cache());

    // Hold many tees open at once. Past the cap they must be refused rather
    // than queued.
    let mut tees = Vec::new();
    for _ in 0..64 {
        tees.push(factory.open(Some(item()), metas(), policy()));
    }

    let disabled = tees
        .iter()
        .filter(|t| matches!(t, CacheTee::Disabled(TeeDisabled::AtCapacity)))
        .count();
    assert!(
        disabled > 0,
        "past the concurrency cap, tees should be refused"
    );
}
