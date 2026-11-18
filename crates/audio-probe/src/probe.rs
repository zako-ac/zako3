//! Per-stream counters and gate times.
//!
//! One [`StreamProbe`] belongs to one audio stream from the moment the engine
//! asks for it until the transfer ends. Everything in it is an atomic read, so
//! the hot path (one frame every 20 ms) costs two relaxed increments and a
//! mutex-free timestamp.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{now_unix_ms, report::GapStats};

/// taphub read a chunk from the tap (or from the audio cache).
pub const HOP_TAP_TO_TAPHUB: &str = "tap_to_taphub";
/// taphub handed a chunk to the transport toward the engine.
pub const HOP_TAPHUB_TO_ENGINE: &str = "taphub_to_engine";
/// The engine read and parsed a chunk off the socket.
pub const HOP_ENGINE_RECV: &str = "engine_recv";
/// The jitter buffer accepted a frame.
pub const HOP_BUFFER_INSERT: &str = "buffer_insert";

/// A gap at or above this many milliseconds is counted separately.
pub const GAP_BUCKET_100MS: u64 = 100;
/// A gap at or above this many milliseconds is counted separately.
pub const GAP_BUCKET_1S: u64 = 1000;

/// Counters and gate times for one audio stream.
///
/// The two counters that matter most are kept explicitly apart: `socket_recv`
/// counts frames the engine read and parsed off the transport, and the jitter
/// buffer owns its own `inserted` count (see [`crate::BufferView`]). A report
/// carrying both answers the question the audit could not: whether the frames
/// were lost on the wire or the engine simply did not buffer them.
pub struct StreamProbe {
    stream: String,
    started_unix_ms: u64,
    socket_recv: AtomicU64,
    injected_drops: AtomicU64,
    engine_recv_last_unix_ms: AtomicU64,
    gap_last_unix_ms: AtomicU64,
    gap_count: AtomicU64,
    gap_max_ms: AtomicU64,
    gap_over_100ms: AtomicU64,
    gap_over_1000ms: AtomicU64,
}

/// What a probe looked like at one instant.
///
/// Deliberately a value rather than a live handle: the stall report is assembled
/// after a round trip to another process, and reading the probe *after* that
/// would describe a later moment than the stall and could blame the wrong hop.
#[derive(Debug, Clone, Default)]
pub struct ProbeSnapshot {
    pub stream: String,
    pub started_unix_ms: u64,
    pub socket_recv: u64,
    pub injected_drops: u64,
    pub engine_recv_last_unix_ms: u64,
    pub gaps: GapStats,
}

impl StreamProbe {
    pub fn new(stream: impl Into<String>) -> Self {
        Self {
            stream: stream.into(),
            started_unix_ms: now_unix_ms(),
            socket_recv: AtomicU64::new(0),
            injected_drops: AtomicU64::new(0),
            engine_recv_last_unix_ms: AtomicU64::new(0),
            gap_last_unix_ms: AtomicU64::new(0),
            gap_count: AtomicU64::new(0),
            gap_max_ms: AtomicU64::new(0),
            gap_over_100ms: AtomicU64::new(0),
            gap_over_1000ms: AtomicU64::new(0),
        }
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    pub fn started_unix_ms(&self) -> u64 {
        self.started_unix_ms
    }

    /// A frame was read and parsed off the socket.
    ///
    /// This is the socket-receive counter. It is incremented before anything
    /// downstream can drop the frame, which is what makes the divergence from
    /// the buffer's `inserted` count meaningful.
    pub fn on_socket_frame(&self) {
        self.on_socket_frame_at(now_unix_ms());
    }

    /// [`Self::on_socket_frame`] with an explicit clock, for tests.
    pub fn on_socket_frame_at(&self, now_ms: u64) {
        self.socket_recv.fetch_add(1, Ordering::Relaxed);
        self.engine_recv_last_unix_ms
            .store(now_ms, Ordering::Relaxed);

        // Atomics rather than a lock: this runs on the frame path, fifty times a
        // second per stream, and a poisoned lock there would take the pump with
        // it. The max is updated with a compare-exchange loop because a lost
        // update here would hide exactly the gap the device exists to find.
        let previous = self.gap_last_unix_ms.swap(now_ms, Ordering::Relaxed);
        if previous != 0 && now_ms > previous {
            let gap = now_ms - previous;
            self.gap_count.fetch_add(1, Ordering::Relaxed);
            let mut current = self.gap_max_ms.load(Ordering::Relaxed);
            while gap > current {
                match self.gap_max_ms.compare_exchange_weak(
                    current,
                    gap,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(observed) => current = observed,
                }
            }
            if gap >= GAP_BUCKET_100MS {
                self.gap_over_100ms.fetch_add(1, Ordering::Relaxed);
            }
            if gap >= GAP_BUCKET_1S {
                self.gap_over_1000ms.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// A frame was discarded on purpose by a synthetic stall injection.
    ///
    /// Counted apart from a real loss so the self-test can prove the injection
    /// fired, and so a report from an investigation window is not mistaken for
    /// organic trouble.
    pub fn on_injected_drop(&self) {
        self.injected_drops.fetch_add(1, Ordering::Relaxed);
    }

    pub fn socket_recv(&self) -> u64 {
        self.socket_recv.load(Ordering::Relaxed)
    }

    pub fn injected_drops(&self) -> u64 {
        self.injected_drops.load(Ordering::Relaxed)
    }

    pub fn engine_recv_last_unix_ms(&self) -> u64 {
        self.engine_recv_last_unix_ms.load(Ordering::Relaxed)
    }

    pub fn gaps(&self) -> GapStats {
        GapStats {
            count: self.gap_count.load(Ordering::Relaxed),
            max_gap_ms: self.gap_max_ms.load(Ordering::Relaxed),
            over_100ms: self.gap_over_100ms.load(Ordering::Relaxed),
            over_1000ms: self.gap_over_1000ms.load(Ordering::Relaxed),
            last_arrival_unix_ms: self.gap_last_unix_ms.load(Ordering::Relaxed),
        }
    }

    /// Everything a stall report needs, read at one instant.
    pub fn snapshot(&self) -> ProbeSnapshot {
        ProbeSnapshot {
            stream: self.stream.clone(),
            started_unix_ms: self.started_unix_ms,
            socket_recv: self.socket_recv(),
            injected_drops: self.injected_drops(),
            engine_recv_last_unix_ms: self.engine_recv_last_unix_ms(),
            gaps: self.gaps(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_are_bucketed_and_reported() {
        let probe = StreamProbe::new("test");
        probe.on_socket_frame_at(1_000);
        probe.on_socket_frame_at(1_020);
        probe.on_socket_frame_at(1_140); // 120 ms gap
        probe.on_socket_frame_at(3_140); // 2000 ms gap

        let gaps = probe.gaps();
        assert_eq!(probe.socket_recv(), 4);
        assert_eq!(gaps.count, 3);
        assert_eq!(gaps.max_gap_ms, 2_000);
        assert_eq!(gaps.over_100ms, 2);
        assert_eq!(gaps.over_1000ms, 1);
        assert_eq!(gaps.last_arrival_unix_ms, 3_140);
        assert_eq!(probe.engine_recv_last_unix_ms(), 3_140);
    }

    #[test]
    fn first_frame_never_counts_as_a_gap() {
        let probe = StreamProbe::new("test");
        probe.on_socket_frame_at(1_000);
        let gaps = probe.gaps();
        assert_eq!(gaps.count, 0);
        assert_eq!(gaps.max_gap_ms, 0);
    }
}
