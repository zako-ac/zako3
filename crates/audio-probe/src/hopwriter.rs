//! The taphub side of the hop record.
//!
//! taphub sees two of the four hops — the chunk arriving from the tap (or from
//! the audio cache) and the chunk being handed to the transport — and the engine
//! sees neither. This writer keeps taphub's two gate times in atomics and hands
//! out a snapshot; the caller publishes the snapshot to the shared store.
//!
//! Publishing is rate-limited by [`UpstreamHopWriter::should_publish`]: a stream
//! carries 50 frames a second and the record only needs to be a few hundred
//! milliseconds fresh, so one write per stream per interval is plenty. The
//! stream id is the only per-stream key, and it dies with its TTL, so the
//! cardinality is bounded by concurrent streams rather than by history.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    now_unix_ms,
    report::{HOP_RECORD_PREFIX, HOP_RECORD_TTL_SECS, UpstreamHops},
};

/// How often a hop record is republished while frames flow.
pub const PUBLISH_INTERVAL_MS: u64 = 200;

/// Where the taphub-side hop record for a stream lives.
pub fn hop_record_key(stream: &str) -> String {
    format!("{HOP_RECORD_PREFIX}{stream}")
}

pub fn hop_record_ttl_secs() -> u64 {
    HOP_RECORD_TTL_SECS
}

pub struct UpstreamHopWriter {
    stream: String,
    tap_id: String,
    path: &'static str,
    tap_recv_unix_ms: AtomicU64,
    send_unix_ms: AtomicU64,
    frames_from_tap: AtomicU64,
    frames_sent: AtomicU64,
    last_publish_unix_ms: AtomicU64,
}

impl UpstreamHopWriter {
    /// `path` is `tap` or `cache`; it is what tells the reader later whether the
    /// first hop was a tap read at all.
    pub fn new(stream: impl Into<String>, tap_id: impl Into<String>, path: &'static str) -> Self {
        Self {
            stream: stream.into(),
            tap_id: tap_id.into(),
            path,
            tap_recv_unix_ms: AtomicU64::new(0),
            send_unix_ms: AtomicU64::new(0),
            frames_from_tap: AtomicU64::new(0),
            frames_sent: AtomicU64::new(0),
            last_publish_unix_ms: AtomicU64::new(0),
        }
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    /// taphub read one chunk from the tap (or from the cache).
    pub fn on_source_frame(&self) {
        self.tap_recv_unix_ms
            .store(now_unix_ms(), Ordering::Relaxed);
        self.frames_from_tap.fetch_add(1, Ordering::Relaxed);
    }

    /// taphub handed one chunk to the transport toward the engine.
    pub fn on_send(&self) {
        self.send_unix_ms.store(now_unix_ms(), Ordering::Relaxed);
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> UpstreamHops {
        UpstreamHops {
            stream: self.stream.clone(),
            path: self.path.to_string(),
            tap_id: self.tap_id.clone(),
            tap_recv_unix_ms: self.tap_recv_unix_ms.load(Ordering::Relaxed),
            send_unix_ms: self.send_unix_ms.load(Ordering::Relaxed),
            frames_from_tap: self.frames_from_tap.load(Ordering::Relaxed),
            frames_sent: self.frames_sent.load(Ordering::Relaxed),
            updated_unix_ms: now_unix_ms(),
        }
    }

    /// True at most once per [`PUBLISH_INTERVAL_MS`], whichever frame asks.
    ///
    /// A compare-and-swap rather than a load-then-store so two request tasks
    /// sharing a writer cannot both publish in the same window.
    pub fn should_publish(&self) -> bool {
        let now = now_unix_ms();
        let last = self.last_publish_unix_ms.load(Ordering::Relaxed);
        if now.saturating_sub(last) < PUBLISH_INTERVAL_MS && last != 0 {
            return false;
        }
        self.last_publish_unix_ms
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reflects_the_hops_it_saw() {
        let writer = UpstreamHopWriter::new("s1", "tap1", "tap");
        writer.on_source_frame();
        writer.on_source_frame();
        writer.on_send();

        let snap = writer.snapshot();
        assert_eq!(snap.frames_from_tap, 2);
        assert_eq!(snap.frames_sent, 1);
        assert_eq!(snap.path, "tap");
        assert!(snap.send_unix_ms > 0);
        assert!(snap.tap_recv_unix_ms > 0);
    }

    #[test]
    fn publishing_is_rate_limited_and_never_starves() {
        let writer = UpstreamHopWriter::new("s1", "tap1", "tap");
        assert!(writer.should_publish(), "the first frame publishes");
        assert!(
            !writer.should_publish(),
            "the next frame inside the window does not"
        );
        std::thread::sleep(std::time::Duration::from_millis(PUBLISH_INTERVAL_MS + 20));
        assert!(
            writer.should_publish(),
            "a frame after the window publishes"
        );
    }

    #[test]
    fn the_key_is_namespaced_and_the_ttl_bounded() {
        assert_eq!(hop_record_key("abc"), "audio:probe:hop:abc");
        assert!(hop_record_ttl_secs() >= 60);
    }
}
