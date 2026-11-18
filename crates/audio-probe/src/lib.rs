//! Hop-by-hop stall instrumentation for the audio path.
//!
//! The audit that prompted this crate could say "the tap was fine, taphub logged
//! the chunk, and the engine had zero frames for fifteen seconds" and no more,
//! because nothing in the pipeline recorded *where* a frame stopped. This crate
//! is the answer: per-stream counters that separate "read off the socket" from
//! "inserted into the jitter buffer", a last-frame wall-clock time per hop, and
//! a single structured line emitted when a stream stalls, carrying all of it
//! plus a verdict naming the hop the break is in.
//!
//! The four hops, in pipeline order:
//!
//! | hop | recorded by | means |
//! |---|---|---|
//! | [`HOP_TAP_TO_TAPHUB`] | taphub | taphub read a chunk from the tap (or the cache) |
//! | [`HOP_TAPHUB_TO_ENGINE`] | taphub | taphub handed that chunk to the transport toward the engine |
//! | [`HOP_ENGINE_RECV`] | engine | the engine read and parsed a chunk off the socket |
//! | [`HOP_BUFFER_INSERT`] | engine | the jitter buffer accepted a frame |
//!
//! The last two are what make the ambiguity go away: if `engine_recv` keeps
//! advancing while `buffer_insert` does not, the frames arrived and the engine
//! failed to buffer them. If both stand still, they never arrived.
//!
//! Hops 1 and 2 live in the taphub process, so they reach the engine through a
//! short-TTL Redis record keyed by stream ([`UpstreamHopWriter`] writes it,
//! [`UpstreamHopSource`] reads it). When the record is missing the report says
//! so rather than guessing.

mod hopwriter;
mod inject;
mod metrics;
mod probe;
mod report;
mod trace;

pub use hopwriter::{UpstreamHopWriter, hop_record_key, hop_record_ttl_secs};
pub use inject::{
    Stage, clear as clear_injection, dropping, init_from_env as init_injection_from_env, inject,
};
pub use metrics::record_injected_drop;
pub use probe::{
    GAP_BUCKET_1S, GAP_BUCKET_100MS, HOP_BUFFER_INSERT, HOP_ENGINE_RECV, HOP_TAP_TO_TAPHUB,
    HOP_TAPHUB_TO_ENGINE, StreamProbe,
};
pub use report::{
    BufferView, CountersView, DEFAULT_FRESH_MS, GapStats, HOP_RECORD_PREFIX, HOP_RECORD_TTL_SECS,
    HopAges, HopView, RequestView, StallInputs, StallReport, UpstreamHopSource, UpstreamHops,
    Verdict, build_report, classify, emit, fresh_threshold_ms,
};
pub use trace::{StreamTrace, current_trace, with_trace};

/// Wall-clock milliseconds since the Unix epoch.
///
/// Every hop stamps its last frame with this rather than a monotonic clock, so
/// timestamps from different pods are comparable on one axis. Node clocks are
/// NTP-disciplined; a skew between the taphub and engine nodes shows up directly
/// as a nonsensical age in a report, which is itself worth seeing.
pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The stream id both sides derive for one audio request.
///
/// Deliberately derivable from the request alone — tap id plus the request
/// string — so taphub and the engine agree on it without a protocol change and
/// without either side telling the other. Sixteen hex characters of SHA-256 is
/// far more than enough to keep concurrent requests apart.
pub fn request_stream_id(tap_id: &str, discord_user_id: &str, audio_request: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(discord_user_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(audio_request.as_bytes());
    let hash = hex::encode(hasher.finalize());
    format!("{}-{}", tap_id, &hash[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_id_is_stable_and_input_sensitive() {
        let a = request_stream_id("340376482209599488", "1", "hello world");
        let b = request_stream_id("340376482209599488", "1", "hello world");
        let c = request_stream_id("340376482209599488", "1", "hello worle");
        let d = request_stream_id("340376482209599488", "2", "hello world");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d, "two users asking for the same text are two streams");
        assert!(a.starts_with("340376482209599488-"));
        assert_eq!(a.len(), 18 + 1 + 16);
    }
}
