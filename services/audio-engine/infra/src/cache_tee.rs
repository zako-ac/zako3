//! Writing a copy of the audio to the cache, without ever letting that block
//! playback.
//!
//! The requirement is one-directional: the cache may learn about playback, but
//! playback must never wait on the cache. That is enforced structurally rather
//! than by care — the tee holds no channel back into the session task, so there
//! is no path by which it could apply backpressure even if someone added an
//! `await` in the wrong place.
//!
//! The shape being avoided is taphub's: `bridge_rel` does `tx.send(..).await`
//! into a bounded channel feeding a streaming HTTP body, so a cache server that
//! stops reading fills the channel and blocks the sender indefinitely. That
//! only ever wedged a dedicated task there, which is why it has not bitten yet.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::mpsc;
use zako3_cache_client::RemoteAudioCache;
use zako3_types::cache::AudioCacheItem;
use zako3_types::{AudioCachePolicy, AudioMetadata};

/// Frames held between the receiver and the uploader.
///
/// Deliberately shallow. A backlog here means the cache server is not keeping
/// up, and queueing deeper only delays discovering that while holding more
/// memory per concurrent request.
const TEE_QUEUE: usize = 256;

/// Concurrent uploads one engine will attempt.
const MAX_CONCURRENT_UPLOADS: usize = 32;

/// Consecutive failures before the tee stops trying.
///
/// Without this, a dead cache server costs *every* request a connect timeout,
/// so the failure scales with traffic instead of being absorbed.
const BREAKER_THRESHOLD: u32 = 5;

/// How long the breaker stays open before probing again.
const BREAKER_COOLDOWN: Duration = Duration::from_secs(30);

/// Why a tee is not running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeeDisabled {
    /// The request is not cacheable at all.
    NotCacheable,
    /// The cache server is failing and the breaker is open.
    CircuitOpen,
    /// Too many uploads already in flight on this engine.
    AtCapacity,
}

/// A running tee, or the reason there isn't one.
///
/// Modelled as a value rather than an `Option<Sender>` so that "no tee" is a
/// state the caller has to acknowledge, and so no code path can accidentally
/// await one that does not exist.
pub enum CacheTee {
    Active(ActiveTee),
    Disabled(TeeDisabled),
}

impl CacheTee {
    /// Offer a frame to the cache.
    ///
    /// Never waits and never fails upward: a full queue drops the frame and
    /// poisons the upload, because a partial cache entry is worthless and
    /// stalling playback to avoid one is far worse.
    pub fn offer(&self, frame: Bytes) {
        if let CacheTee::Active(tee) = self {
            tee.offer(frame);
        }
    }

    /// Signal that the reliable stream completed, so the entry may be committed.
    ///
    /// Anything else — dropping this without calling it — aborts, which is the
    /// safe default: a truncated entry must never be committed.
    pub fn commit(self) {
        if let CacheTee::Active(tee) = self {
            tee.commit();
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, CacheTee::Active(_))
    }
}

pub struct ActiveTee {
    frames: mpsc::Sender<Bytes>,
    complete: tokio::sync::oneshot::Sender<()>,
    dropped: Arc<AtomicU64>,
}

impl ActiveTee {
    fn offer(&self, frame: Bytes) {
        if self.frames.try_send(frame).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn commit(self) {
        let _ = self.complete.send(());
    }
}

/// Opens tees, and remembers when the cache server is unwell.
#[derive(Clone)]
pub struct CacheTeeFactory {
    cache: Arc<RemoteAudioCache>,
    uploads: Arc<tokio::sync::Semaphore>,
    consecutive_failures: Arc<AtomicU64>,
    /// When the breaker opened, as a monotonic instant in an atomic-friendly
    /// form. Zero means closed.
    opened_at: Arc<std::sync::Mutex<Option<tokio::time::Instant>>>,
    aborts: Arc<AtomicU64>,
}

impl CacheTeeFactory {
    pub fn new(cache: Arc<RemoteAudioCache>) -> Self {
        Self {
            cache,
            uploads: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_UPLOADS)),
            consecutive_failures: Arc::new(AtomicU64::new(0)),
            opened_at: Arc::new(std::sync::Mutex::new(None)),
            aborts: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Tees that ended without committing.
    ///
    /// The number worth alerting on: a cache quietly refusing everything looks
    /// exactly like a healthy one from the playback side.
    pub fn aborted_count(&self) -> u64 {
        self.aborts.load(Ordering::Relaxed)
    }

    fn breaker_open(&self) -> bool {
        let mut opened = self.opened_at.lock().expect("breaker mutex");
        match *opened {
            Some(at) if at.elapsed() < BREAKER_COOLDOWN => true,
            Some(_) => {
                // Cooldown elapsed: let one request through to probe.
                *opened = None;
                self.consecutive_failures.store(0, Ordering::Relaxed);
                false
            }
            None => false,
        }
    }

    fn record_failure(&self) {
        let n = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= BREAKER_THRESHOLD as u64 {
            *self.opened_at.lock().expect("breaker mutex") = Some(tokio::time::Instant::now());
            tracing::warn!(
                failures = n,
                cooldown_secs = BREAKER_COOLDOWN.as_secs(),
                "cache server is failing; pausing the cache tee"
            );
        }
    }

    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// Start teeing for one request.
    ///
    /// Returns immediately: the upload session is opened inside the spawned
    /// task, so playback never waits on the cache server even to find out
    /// whether caching is possible.
    pub fn open(
        &self,
        item: Option<AudioCacheItem>,
        metadatas: Vec<AudioMetadata>,
        policy: AudioCachePolicy,
    ) -> CacheTee {
        let Some(item) = item else {
            return CacheTee::Disabled(TeeDisabled::NotCacheable);
        };
        if self.breaker_open() {
            return CacheTee::Disabled(TeeDisabled::CircuitOpen);
        }
        let Ok(permit) = Arc::clone(&self.uploads).try_acquire_owned() else {
            return CacheTee::Disabled(TeeDisabled::AtCapacity);
        };

        let (frames, frame_rx) = mpsc::channel(TEE_QUEUE);
        let (complete, complete_rx) = tokio::sync::oneshot::channel();
        let dropped = Arc::new(AtomicU64::new(0));

        let cache = Arc::clone(&self.cache);
        let this = self.clone();
        let dropped_for_task = Arc::clone(&dropped);

        tokio::spawn(async move {
            let _permit = permit;
            use zako3_preload_cache::AudioCache;

            // `store` commits only if `complete_rx` resolves; dropping the
            // sender aborts. That is the whole correctness argument for the
            // tee, so it stays exactly as the cache client defines it.
            let result = cache
                .store(item.clone(), metadatas, policy, frame_rx, complete_rx)
                .await;

            let lost = dropped_for_task.load(Ordering::Relaxed);
            match result {
                Ok(()) if lost == 0 => {
                    this.record_success();
                    tracing::debug!(tap_id = %item.tap_id.0, key = %item.key, "cached");
                }
                Ok(()) => {
                    // Committed despite dropped frames would mean a truncated
                    // entry, so treat it as a failure of this tee even though
                    // the upload itself succeeded.
                    this.aborts.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        tap_id = %item.tap_id.0,
                        dropped_frames = lost,
                        "cache tee fell behind; entry not trustworthy"
                    );
                }
                Err(e) => {
                    this.aborts.fetch_add(1, Ordering::Relaxed);
                    this.record_failure();
                    tracing::warn!(%e, tap_id = %item.tap_id.0, "cache tee failed");
                }
            }
        });

        CacheTee::Active(ActiveTee { frames, complete, dropped })
    }
}
