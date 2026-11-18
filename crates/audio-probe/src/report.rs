//! The engine's view of the upstream hops, and the report that names a break.
//!
//! A [`StallReport`] is the one structured line the device exists to produce.
//! It is assembled at the moment the jitter buffer gives up waiting, and it
//! carries, in a single JSON object:
//!
//! - the stream, request and (when the caller set one) track id;
//! - the last frame wall-clock time at each hop, with its age;
//! - the separably-counted socket-receive and buffer-insert counters;
//! - frame arrival gaps (max, and how many exceeded 100 ms / 1 s);
//! - the jitter buffer's own state (depth, capacity, play head, late, dropped,
//!   concealed);
//! - a verdict naming the hop the break is in, and the reasoning in words.
//!
//! The verdict is a pure function of those numbers ([`classify`]) so it can be
//! tested against each shape of break without a real pipeline.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    HOP_BUFFER_INSERT, HOP_ENGINE_RECV, HOP_TAP_TO_TAPHUB, HOP_TAPHUB_TO_ENGINE,
    probe::ProbeSnapshot, trace::StreamTrace,
};

/// Redis key prefix for the taphub-written hop record.
pub const HOP_RECORD_PREFIX: &str = "audio:probe:hop:";
/// How long a hop record lives without being refreshed.
pub const HOP_RECORD_TTL_SECS: u64 = 120;

/// The two hops only taphub can see, keyed by stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpstreamHops {
    pub stream: String,
    /// `tap` when the audio came from the tap, `cache` when taphub served it
    /// from its audio cache. A cache stream has no tap hop at all, so a stalled
    /// cache stream reports the cache read and the send, not a silent tap.
    pub path: String,
    pub tap_id: String,
    pub tap_recv_unix_ms: u64,
    pub send_unix_ms: u64,
    pub frames_from_tap: u64,
    pub frames_sent: u64,
    pub updated_unix_ms: u64,
}

/// Something that can hand the engine a stream's upstream hop record.
///
/// Implemented over Redis in the engine; an in-memory map in tests.
#[async_trait::async_trait]
pub trait UpstreamHopSource: Send + Sync {
    async fn read(&self, stream: &str) -> Option<UpstreamHops>;
}

/// Frame arrival gaps at the engine's socket.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct GapStats {
    pub count: u64,
    pub max_gap_ms: u64,
    pub over_100ms: u64,
    pub over_1000ms: u64,
    #[serde(skip)]
    pub last_arrival_unix_ms: u64,
}

/// What the request behind a stream was.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RequestView {
    pub tap_id: String,
    /// The cache policy the request carried (`none`, `ar_hash`, `key:<k>`), not
    /// a cache key: the engine does not hold one at this point.
    pub cache_policy: Option<String>,
    /// First 16 hex characters of the request string's SHA-256 — the same value
    /// [`crate::request_stream_id`] embeds, so a log line can find the stream.
    pub ars_hash: String,
    pub ars_len: usize,
}

/// The jitter buffer's state at the moment of the stall.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BufferView {
    pub depth: usize,
    pub capacity: usize,
    pub max_lead_frames: usize,
    pub buffered_ms: u64,
    pub playhead_ms: Option<u64>,
    pub inserted: u64,
    pub dropped: u64,
    pub late: u64,
    pub concealed: u64,
    pub stall_events: u64,
    pub last_insert_unix_ms: u64,
}

/// The two counters that must never be conflated.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CountersView {
    /// Frames read and parsed off the socket by the engine.
    pub socket_recv_frames: u64,
    /// Frames the jitter buffer accepted.
    pub buffer_inserted_frames: u64,
    /// `socket_recv_frames - buffer_inserted_frames`. Positive means frames
    /// reached the process but never reached the buffer.
    pub socket_minus_inserted: i64,
    /// Frames discarded on purpose by [`crate::inject`].
    pub injected_drops: u64,
}

/// Last-frame age per hop, as used by the classifier.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct HopAges {
    pub tap_to_taphub: Option<u64>,
    pub taphub_to_engine: Option<u64>,
    pub engine_recv: Option<u64>,
    pub buffer_insert: Option<u64>,
}

/// One hop's line in the report.
#[derive(Debug, Clone, Serialize)]
pub struct HopView {
    pub last_frame_unix_ms: Option<u64>,
    pub age_ms: Option<u64>,
    pub frames: Option<u64>,
    /// `engine`, `taphub` or `unavailable`.
    pub source: &'static str,
}

/// Which segment of the pipeline the break is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Frames reach the jitter buffer, yet playback waits: the break is below
    /// the transport (mixer, decoder, output).
    EngineDownstream,
    /// Frames are being read but the buffer's lead window is full — the engine
    /// is holding them back on purpose.
    EngineBackpressure,
    /// Frames were read off the socket but never reached the jitter buffer.
    EngineInternal,
    /// taphub was still handing frames to the transport while the engine's
    /// socket went quiet: the loss is on the path between them.
    TaphubToEnginePath,
    /// taphub's own read from the tap stopped while it was still sending
    /// earlier frames: the loss is between the tap and taphub.
    TapToTaphubPath,
    /// Nothing upstream of the engine moved: the tap (or the cache reader) is
    /// the quiet end.
    TapOrCache,
    /// The evidence is not enough to name a hop.
    Unknown,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::EngineDownstream => "engine_downstream",
            Verdict::EngineBackpressure => "engine_backpressure",
            Verdict::EngineInternal => "engine_internal",
            Verdict::TaphubToEnginePath => "taphub_to_engine_path",
            Verdict::TapToTaphubPath => "tap_to_taphub_path",
            Verdict::TapOrCache => "tap_or_cache",
            Verdict::Unknown => "unknown",
        }
    }
}

/// The structured stall line.
#[derive(Debug, Clone, Serialize)]
pub struct StallReport {
    pub event: &'static str,
    pub ts_unix_ms: u64,
    pub stream: String,
    pub tap_id: String,
    pub track_id: Option<String>,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub request: RequestView,
    pub stalled_ms: u64,
    pub stream_age_ms: u64,
    pub verdict: Verdict,
    pub verdict_reason: String,
    /// Ages at or under this many milliseconds count as "still moving".
    pub fresh_threshold_ms: u64,
    pub hops: BTreeMap<&'static str, HopView>,
    pub ages: HopAges,
    pub counters: CountersView,
    pub gaps: GapStats,
    pub buffer: BufferView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<UpstreamHops>,
}

/// Everything needed to assemble one report.
pub struct StallInputs {
    pub stall_ms: u64,
    /// Captured when the stall was detected, not when the report was assembled:
    /// assembling one reads another process, and a probe read afterwards would
    /// describe a later instant than the one being explained.
    pub now_unix_ms: u64,
    pub probe: ProbeSnapshot,
    pub buffer: BufferView,
    pub trace: Option<StreamTrace>,
    pub request: RequestView,
    pub upstream: Option<UpstreamHops>,
}

/// Ages at or under this count as "still moving" in a report.
///
/// A frame arrives every 20 ms, so a hop quiet for two seconds is not merely
/// late, it is stopped; anything fresher is treated as alive so a jittery clock
/// or a slow sample does not flip the verdict.
pub const DEFAULT_FRESH_MS: u64 = 2_000;

/// How stale a hop has to be, for *this* stall, to count as stopped.
///
/// A quarter of the wait, clamped: the question is whether a hop stopped before
/// playback did, so the line has to sit well inside the stall rather than at a
/// fixed two seconds. With the production 15 s timeout this is 2 s; with a short
/// timeout in a test it is correspondingly short, and the verdict means the same
/// thing either way.
pub fn fresh_threshold_ms(stalled_ms: u64) -> u64 {
    (stalled_ms / 4).clamp(50, DEFAULT_FRESH_MS)
}

fn age(now: u64, then: u64) -> Option<u64> {
    (then != 0).then(|| now.saturating_sub(then))
}

pub fn build_report(inputs: StallInputs) -> StallReport {
    let StallInputs {
        stall_ms,
        now_unix_ms,
        probe,
        buffer,
        trace,
        mut request,
        upstream,
    } = inputs;

    let stream = probe.stream.clone();
    if request.tap_id.is_empty() {
        request.tap_id = stream.split('-').next().unwrap_or_default().to_string();
    }

    let ages = HopAges {
        tap_to_taphub: upstream
            .as_ref()
            .and_then(|u| age(now_unix_ms, u.tap_recv_unix_ms)),
        taphub_to_engine: upstream
            .as_ref()
            .and_then(|u| age(now_unix_ms, u.send_unix_ms)),
        engine_recv: age(now_unix_ms, probe.engine_recv_last_unix_ms),
        buffer_insert: age(now_unix_ms, buffer.last_insert_unix_ms),
    };

    let fresh_ms = fresh_threshold_ms(stall_ms);
    let (verdict, reason) = classify(
        &ages,
        &buffer,
        upstream.is_some(),
        probe.injected_drops,
        fresh_ms,
    );

    let mut hops = BTreeMap::new();
    hops.insert(
        HOP_TAP_TO_TAPHUB,
        HopView {
            last_frame_unix_ms: upstream
                .as_ref()
                .map(|u| u.tap_recv_unix_ms)
                .filter(|v| *v != 0),
            age_ms: ages.tap_to_taphub,
            frames: upstream.as_ref().map(|u| u.frames_from_tap),
            source: if upstream.is_some() {
                "taphub"
            } else {
                "unavailable"
            },
        },
    );
    hops.insert(
        HOP_TAPHUB_TO_ENGINE,
        HopView {
            last_frame_unix_ms: upstream
                .as_ref()
                .map(|u| u.send_unix_ms)
                .filter(|v| *v != 0),
            age_ms: ages.taphub_to_engine,
            frames: upstream.as_ref().map(|u| u.frames_sent),
            source: if upstream.is_some() {
                "taphub"
            } else {
                "unavailable"
            },
        },
    );
    hops.insert(
        HOP_ENGINE_RECV,
        HopView {
            last_frame_unix_ms: Some(probe.engine_recv_last_unix_ms).filter(|v| *v != 0),
            age_ms: ages.engine_recv,
            frames: Some(probe.socket_recv),
            source: "engine",
        },
    );
    hops.insert(
        HOP_BUFFER_INSERT,
        HopView {
            last_frame_unix_ms: Some(buffer.last_insert_unix_ms).filter(|v| *v != 0),
            age_ms: ages.buffer_insert,
            frames: Some(buffer.inserted),
            source: "engine",
        },
    );

    let socket_recv = probe.socket_recv;
    let counters = CountersView {
        socket_recv_frames: socket_recv,
        buffer_inserted_frames: buffer.inserted,
        socket_minus_inserted: socket_recv as i64 - buffer.inserted as i64,
        injected_drops: probe.injected_drops,
    };

    let (track_id, guild_id, channel_id) = match trace {
        Some(t) => (t.track_id, t.guild_id, t.channel_id),
        None => (None, None, None),
    };

    StallReport {
        event: "audio_stream_stall",
        ts_unix_ms: now_unix_ms,
        stream,
        tap_id: request.tap_id.clone(),
        track_id,
        guild_id,
        channel_id,
        request,
        stalled_ms: stall_ms,
        stream_age_ms: now_unix_ms.saturating_sub(probe.started_unix_ms),
        verdict,
        verdict_reason: reason,
        fresh_threshold_ms: fresh_ms,
        hops,
        ages,
        counters,
        gaps: probe.gaps,
        buffer,
        upstream,
    }
}

/// Name the hop the break is in, from last-frame ages and buffer state.
///
/// The rules are ordered so that the most specific, best-evidenced case wins:
/// an engine break that is visible in the engine's own two counters is decided
/// before anything that needs the upstream record, because it does not depend on
/// another process having written anything.
pub fn classify(
    ages: &HopAges,
    buffer: &BufferView,
    upstream_available: bool,
    injected_drops: u64,
    fresh_ms: u64,
) -> (Verdict, String) {
    let fresh = |a: Option<u64>| matches!(a, Some(age) if age <= fresh_ms);

    let engine_recv = ages.engine_recv;
    let insert = ages.buffer_insert;

    // Frames arrive at the jitter buffer and it still stalled: the wait is
    // below the transport.
    if fresh(engine_recv) && fresh(insert) {
        let lead_full = buffer.max_lead_frames > 0 && buffer.depth >= buffer.max_lead_frames;
        if lead_full {
            return (
                Verdict::EngineBackpressure,
                format!(
                    "buffer is at its lead window ({} of {} frames, {} ms buffered) while frames keep arriving: the buffer stopped reading its channel on purpose",
                    buffer.depth, buffer.max_lead_frames, buffer.buffered_ms
                ),
            );
        }
        return (
            Verdict::EngineDownstream,
            format!(
                "frames reached the jitter buffer (last insert {} ms ago) yet playback stalled: the break is below the transport (mixer/decoder/output)",
                insert.unwrap_or(0)
            ),
        );
    }

    // Read off the socket, never buffered.
    if fresh(engine_recv) && !fresh(insert) {
        let injected = if injected_drops > 0 {
            format!(" and {injected_drops} frames were dropped by a synthetic injection")
        } else {
            String::new()
        };
        return (
            Verdict::EngineInternal,
            format!(
                "the engine kept reading the socket (last frame {} ms ago) but the jitter buffer took nothing for {} ms{}: pump/mpsc starvation or backpressure inside the engine",
                engine_recv.unwrap_or(0),
                insert
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "n/a".to_string()),
                injected
            ),
        );
    }

    // Socket quiet from here on: only the upstream record can say more.
    if fresh(ages.taphub_to_engine) {
        return (
            Verdict::TaphubToEnginePath,
            format!(
                "taphub was still handing frames to the transport (last send {} ms ago) while the engine's socket went quiet ({} ms): the loss is between taphub and the engine",
                ages.taphub_to_engine.unwrap_or(0),
                engine_recv
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "n/a".to_string())
            ),
        );
    }

    if fresh(ages.tap_to_taphub) && !fresh(ages.taphub_to_engine) {
        return (
            Verdict::TapToTaphubPath,
            format!(
                "taphub stopped sending (last send {} ms ago) while it was still reading from the tap ({} ms): the loss is between the tap and taphub",
                ages.taphub_to_engine
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "n/a".to_string()),
                ages.tap_to_taphub.unwrap_or(0)
            ),
        );
    }

    if upstream_available {
        return (
            Verdict::TapOrCache,
            "no hop has moved: the tap (or the cache reader) is the quiet end".to_string(),
        );
    }

    (
        Verdict::Unknown,
        format!(
            "the engine's socket went quiet ({} ms) and no upstream hop record was available, so taphub's send time is unknown; check taphub logs for this stream or re-run with the probe reachable",
            engine_recv
                .map(|a| a.to_string())
                .unwrap_or_else(|| "n/a".to_string())
        ),
    )
}

/// Write the stall line and count it.
///
/// Returns the JSON line that was written, so a caller (or a test) can assert on
/// exactly what left the process.
pub fn emit(report: &StallReport) -> String {
    let line = serde_json::to_string(report).unwrap_or_else(|e| {
        format!(r#"{{"event":"audio_stream_stall","error":"serialize failed: {e}"}}"#)
    });

    // One JSON object, alone on its line: the message *is* the object, so a log
    // pipeline that splits on a leading `{` gets a whole record rather than a
    // field buried in a formatted message.
    tracing::warn!(
        target: "zako3::audio::stall",
        verdict = report.verdict.as_str(),
        stream = %report.stream,
        track_id = report.track_id.as_deref().unwrap_or("-"),
        injected = report.counters.injected_drops > 0,
        "{}",
        line
    );

    crate::metrics::record_stall(report.verdict, &report.hops);
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(depth: usize, lead: usize, inserted: u64) -> BufferView {
        BufferView {
            depth,
            capacity: 750,
            max_lead_frames: lead,
            inserted,
            ..Default::default()
        }
    }

    #[test]
    fn frames_arriving_below_the_transport_is_downstream() {
        let ages = HopAges {
            engine_recv: Some(10),
            buffer_insert: Some(20),
            ..Default::default()
        };
        let (verdict, _) = classify(&ages, &buffer(5, 25, 100), false, 0, 2_000);
        assert_eq!(verdict, Verdict::EngineDownstream);
    }

    #[test]
    fn a_full_lead_window_is_backpressure_not_loss() {
        let ages = HopAges {
            engine_recv: Some(10),
            buffer_insert: Some(20),
            ..Default::default()
        };
        let (verdict, _) = classify(&ages, &buffer(25, 25, 100), false, 0, 2_000);
        assert_eq!(verdict, Verdict::EngineBackpressure);
    }

    #[test]
    fn socket_moving_while_the_buffer_is_not_is_engine_internal() {
        let ages = HopAges {
            engine_recv: Some(5),
            buffer_insert: Some(15_000),
            ..Default::default()
        };
        let (verdict, reason) = classify(&ages, &buffer(0, 25, 100), true, 37, 2_000);
        assert_eq!(verdict, Verdict::EngineInternal);
        assert!(reason.contains("37 frames"), "{reason}");
    }

    #[test]
    fn taphub_sending_while_the_engine_is_quiet_is_a_path_loss() {
        let ages = HopAges {
            tap_to_taphub: Some(20),
            taphub_to_engine: Some(30),
            engine_recv: Some(15_000),
            buffer_insert: Some(15_000),
        };
        let (verdict, _) = classify(&ages, &buffer(0, 25, 100), true, 0, 2_000);
        assert_eq!(verdict, Verdict::TaphubToEnginePath);
    }

    #[test]
    fn taphub_quiet_while_the_tap_is_not_is_a_tap_side_path_loss() {
        let ages = HopAges {
            tap_to_taphub: Some(30),
            taphub_to_engine: Some(15_000),
            engine_recv: Some(15_000),
            buffer_insert: Some(15_000),
        };
        let (verdict, _) = classify(&ages, &buffer(0, 25, 100), true, 0, 2_000);
        assert_eq!(verdict, Verdict::TapToTaphubPath);
    }

    #[test]
    fn everything_quiet_names_the_tap() {
        let ages = HopAges {
            tap_to_taphub: Some(15_000),
            taphub_to_engine: Some(15_000),
            engine_recv: Some(15_000),
            buffer_insert: Some(15_000),
        };
        let (verdict, _) = classify(&ages, &buffer(0, 25, 100), true, 0, 2_000);
        assert_eq!(verdict, Verdict::TapOrCache);
    }

    #[test]
    fn a_missing_upstream_record_is_admitted_rather_than_guessed() {
        let ages = HopAges {
            engine_recv: Some(15_000),
            buffer_insert: Some(15_000),
            ..Default::default()
        };
        let (verdict, reason) = classify(&ages, &buffer(0, 25, 100), false, 0, 2_000);
        assert_eq!(verdict, Verdict::Unknown);
        assert!(reason.contains("no upstream hop record"), "{reason}");
    }

    #[test]
    fn the_line_is_one_json_object_with_the_required_fields() {
        let probe = crate::probe::StreamProbe::new("42-abcdef0123456789");
        probe.on_socket_frame_at(980);
        probe.on_socket_frame_at(1_000);
        let mut buf = buffer(0, 25, 1);
        buf.last_insert_unix_ms = 1_000;

        let report = build_report(StallInputs {
            stall_ms: 15_000,
            now_unix_ms: 16_000,
            probe: probe.snapshot(),
            buffer: buf,
            trace: Some(StreamTrace {
                track_id: Some("7522276704182735005".to_string()),
                guild_id: Some("1444631500095094847".to_string()),
                channel_id: Some("123".to_string()),
            }),
            request: RequestView {
                tap_id: "42".to_string(),
                cache_policy: Some("key:ck".to_string()),
                ars_hash: "abcdef0123456789".to_string(),
                ars_len: 11,
            },
            upstream: Some(UpstreamHops {
                stream: "42-abcdef0123456789".to_string(),
                path: "tap".to_string(),
                tap_id: "42".to_string(),
                tap_recv_unix_ms: 1_010,
                send_unix_ms: 1_020,
                frames_from_tap: 50,
                frames_sent: 50,
                updated_unix_ms: 1_020,
            }),
        });

        let line = serde_json::to_string(&report).expect("serializes");
        assert!(!line.contains('\n'), "the stall line must be one line");
        let value: serde_json::Value = serde_json::from_str(&line).expect("valid json");
        for key in [
            "event",
            "stream",
            "track_id",
            "verdict",
            "hops",
            "counters",
            "gaps",
            "buffer",
            "stalled_ms",
        ] {
            assert!(value.get(key).is_some(), "missing {key}: {line}");
        }
        assert_eq!(value["event"], "audio_stream_stall");
        assert_eq!(value["hops"][HOP_ENGINE_RECV]["source"], "engine");
        assert_eq!(value["hops"][HOP_TAPHUB_TO_ENGINE]["source"], "taphub");
        assert_eq!(value["counters"]["socket_recv_frames"], 2);
        assert_eq!(value["counters"]["buffer_inserted_frames"], 1);
        assert_eq!(value["counters"]["socket_minus_inserted"], 1);
        assert_eq!(report.verdict, Verdict::TapOrCache);
    }
}
