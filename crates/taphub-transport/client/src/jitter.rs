//! Feeds a protofish3 transfer into the shared jitter buffer.
//!
//! The buffer itself now lives in `zako3-opus-jitter`, so the protofish4 path
//! uses the same one. What is left here is unwrapping protofish3's framing: its
//! chunks are opaque, so the timestamp travels as an 8-byte prefix the server
//! adds by hand.
//!
//! This is also a hop boundary the stall device needs: a chunk read here has
//! come off the socket, and whether it goes on to reach the buffer is a separate
//! event. Counting both is what separates "the frames never arrived" from "the
//! frames arrived and the engine did not buffer them".

use std::sync::Arc;

use protofish3::xfer::RecvXfer;
use tokio::sync::mpsc;
use zako3_audio_probe::{Stage, StreamProbe, dropping};
use zako3_opus_jitter::TimedFrame;
use zako3_taphub_transport_lib::parse_chunk;

/// Frames buffered between the transfer and the jitter buffer.
///
/// Small: the jitter buffer drains this eagerly and does its own bounding, so a
/// deep queue here would only hide backlog from the frame cap.
pub const CHANNEL_CAP: usize = 64;

/// Handle one chunk read off the transfer.
///
/// Returns false when the consumer is gone and the pump should stop. A frame
/// counted here and then discarded — by a synthetic injection, or by a full
/// channel — has still been *received*, and the socket counter says so; that is
/// the whole point of counting at this end of the hop.
pub async fn pump_chunk(
    data: &[u8],
    tx: &mpsc::Sender<TimedFrame>,
    probe: Option<&StreamProbe>,
) -> bool {
    let Some((ts, body)) = parse_chunk(data) else {
        tracing::warn!(
            len = data.len(),
            "pf3 chunk shorter than its timestamp prefix"
        );
        return true;
    };

    if let Some(probe) = probe {
        probe.on_socket_frame();
        if dropping(probe.stream(), Stage::EnginePump) {
            probe.on_injected_drop();
            zako3_audio_probe::record_injected_drop(Stage::EnginePump, 1);
            return true;
        }
    }

    tx.send(TimedFrame {
        ts_ms: ts.0,
        payload: body.to_vec(),
    })
    .await
    .is_ok()
}

/// Pump a transfer into `tx` until it ends.
///
/// Takes the transfer by reference rather than returning a receiver, because
/// `RecvXfer` borrows the channel receiver it came from and cannot outlive it —
/// so the pump has to run alongside the consumer rather than in a task of its
/// own.
pub async fn pump(
    xfer: &mut RecvXfer<'_>,
    tx: mpsc::Sender<TimedFrame>,
    probe: Option<Arc<StreamProbe>>,
) {
    while let Some(data) = xfer.recv().await {
        if !pump_chunk(&data, &tx, probe.as_deref()).await {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedSender;
    use zako3_audio_probe::{
        BufferView, RequestView, StallInputs, StallReport, StreamTrace, UpstreamHopSource,
        UpstreamHopWriter, UpstreamHops, Verdict, build_report, clear_injection, inject,
        now_unix_ms,
    };
    use zako3_opus_jitter::{JitterConfig, OpusJitterBuffer, StallEvent};
    use zako3_taphub_transport_lib::{Timestamp, encode_chunk};

    const FRAME_MS: u64 = 20;

    /// Encoded silence, so the decoder has a real packet to decode.
    fn frames(n: usize) -> Vec<Vec<u8>> {
        let mut enc = opus::Encoder::new(48_000, opus::Channels::Stereo, opus::Application::Audio)
            .expect("encoder");
        let pcm = vec![0f32; 960 * 2];
        (0..n)
            .map(|_| enc.encode_vec_float(&pcm, 4000).expect("encode"))
            .collect()
    }

    fn chunk(i: usize, payload: &[u8]) -> Vec<u8> {
        encode_chunk(Timestamp(i as u64 * FRAME_MS), payload)
    }

    fn client_cfg() -> JitterConfig {
        JitterConfig {
            stall_timeout: Duration::from_millis(300),
            ..Default::default()
        }
    }

    /// An in-memory stand-in for the Redis record taphub publishes.
    #[derive(Clone, Default)]
    struct MemHops(Arc<Mutex<Option<UpstreamHops>>>);

    impl MemHops {
        fn publish(&self, writer: &UpstreamHopWriter) {
            *self.0.lock().expect("hops") = Some(writer.snapshot());
        }
    }

    #[async_trait::async_trait]
    impl UpstreamHopSource for MemHops {
        async fn read(&self, _stream: &str) -> Option<UpstreamHops> {
            self.0.lock().expect("hops").clone()
        }
    }

    fn build(
        probe: &StreamProbe,
        event: StallEvent,
        upstream: Option<UpstreamHops>,
    ) -> StallReport {
        build_report(StallInputs {
            stall_ms: event.stalled_ms,
            now_unix_ms: now_unix_ms(),
            probe: probe.snapshot(),
            buffer: BufferView {
                depth: event.stats.depth,
                capacity: event.stats.capacity,
                max_lead_frames: event.stats.max_lead_frames,
                buffered_ms: event.stats.buffered_ms,
                playhead_ms: event.stats.playhead_ms,
                inserted: event.stats.inserted,
                dropped: event.stats.dropped,
                late: event.stats.late,
                concealed: event.stats.concealed,
                stall_events: event.stats.stall_events,
                last_insert_unix_ms: event.stats.last_insert_unix_ms,
            },
            trace: Some(StreamTrace::new(
                "7522276704182735005",
                "1444631500095094847",
                "1",
            )),
            request: RequestView {
                tap_id: "42".to_string(),
                cache_policy: None,
                ars_hash: "0f0f0f0f0f0f0f0f".to_string(),
                ars_len: 11,
            },
            upstream,
        })
    }

    fn hops(send_ago_ms: u64, tap_ago_ms: u64, stream: &str) -> UpstreamHops {
        let now = now_unix_ms();
        UpstreamHops {
            stream: stream.to_string(),
            path: "tap".to_string(),
            tap_id: "42".to_string(),
            tap_recv_unix_ms: now.saturating_sub(tap_ago_ms),
            send_unix_ms: now.saturating_sub(send_ago_ms),
            frames_from_tap: 500,
            frames_sent: 500,
            updated_unix_ms: now,
        }
    }

    /// One stream: a producer pushing chunks through the pump, and a consumer
    /// playing the buffer, exactly as the real `request_audio` does.
    struct Harness {
        probe: Arc<StreamProbe>,
        tx: mpsc::Sender<TimedFrame>,
        stalls: mpsc::UnboundedReceiver<StallEvent>,
        payload: Vec<u8>,
    }

    impl Harness {
        fn new(stream: &str) -> Self {
            let (tx, rx) = mpsc::channel(CHANNEL_CAP);
            let probe = Arc::new(StreamProbe::new(stream));
            let mut jb = OpusJitterBuffer::new(rx, client_cfg()).expect("buffer");
            let (stall_tx, stall_rx): (UnboundedSender<StallEvent>, _) = mpsc::unbounded_channel();
            jb.set_stall_notifier(stall_tx);

            tokio::spawn(async move { while let Ok(Some(_pcm)) = jb.yield_pcm().await {} });

            Self {
                probe,
                tx,
                stalls: stall_rx,
                payload: frames(1).remove(0),
            }
        }

        async fn send(&self, i: usize) -> bool {
            pump_chunk(&chunk(i, &self.payload), &self.tx, Some(&self.probe)).await
        }

        /// Feed `count` frames at the nominal frame rate.
        async fn run(&self, from: usize, count: usize) {
            for i in from..from + count {
                if !self.send(i).await {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(FRAME_MS)).await;
            }
        }

        async fn next_stall(&mut self, within: Duration) -> Option<StallEvent> {
            tokio::time::timeout(within, self.stalls.recv())
                .await
                .ok()
                .flatten()
        }
    }

    #[tokio::test]
    async fn a_healthy_stream_raises_nothing() {
        clear_injection();
        let mut h = Harness::new("healthy");
        h.run(0, 50).await; // 1 s of audio, never interrupted
        assert!(
            h.next_stall(Duration::from_millis(250)).await.is_none(),
            "a stream that never went quiet must not report a stall"
        );
    }

    /// The engine reads frames off the socket and never buffers them: the
    /// socket counter must keep climbing while the buffer's does not, and the
    /// verdict must name the engine rather than the path.
    #[tokio::test]
    async fn injection_at_the_pump_diverges_the_counters_and_blames_the_engine() {
        clear_injection();
        let mut h = Harness::new("engine-internal");
        h.run(0, 20).await; // 400 ms of healthy audio

        let socket_before = h.probe.socket_recv();

        inject(
            "engine-internal",
            Stage::EnginePump,
            Duration::from_millis(700),
        );
        h.run(20, 40).await; // 800 ms of reading-but-discarding
        clear_injection();

        let event = h
            .next_stall(Duration::from_secs(2))
            .await
            .expect("the injected stall must be detected");

        assert!(
            h.probe.socket_recv() > socket_before + 20,
            "frames were still read off the socket"
        );
        assert!(h.probe.injected_drops() >= 20, "the injection fired");

        let report = build(&h.probe, event, None);

        assert!(
            report.counters.socket_recv_frames > report.counters.buffer_inserted_frames,
            "socket {} vs inserted {}",
            report.counters.socket_recv_frames,
            report.counters.buffer_inserted_frames
        );
        assert!(report.counters.socket_minus_inserted > 10);
        assert!(report.counters.injected_drops > 0);

        assert_eq!(
            report.verdict,
            Verdict::EngineInternal,
            "{}",
            report.verdict_reason
        );
        let recv_age = report.ages.engine_recv.expect("engine recv age");
        let insert_age = report.ages.buffer_insert.expect("buffer insert age");
        assert!(recv_age < 300, "the socket kept delivering: {recv_age} ms");
        assert!(insert_age >= 500, "the buffer went quiet: {insert_age} ms");
        assert_eq!(
            report.hops[zako3_audio_probe::HOP_ENGINE_RECV].source,
            "engine"
        );
        assert_eq!(
            report.hops[zako3_audio_probe::HOP_TAPHUB_TO_ENGINE].source,
            "unavailable"
        );
    }

    /// taphub keeps producing and its record keeps advancing, the injection
    /// drops every frame on its way to the engine: the verdict must name the
    /// taphub→engine path, and the socket counter must not move.
    ///
    /// This drives the real `dropping(Stage::TaphubSend)` predicate and a real
    /// `UpstreamHopWriter`, so it fails if either half drifts from the other.
    #[tokio::test]
    async fn injection_at_taphub_send_leaves_the_engine_quiet_and_blames_the_path() {
        clear_injection();
        let mut h = Harness::new("path-loss");
        h.run(0, 20).await; // 400 ms of healthy audio first

        let socket_before = h.probe.socket_recv();
        let writer = Arc::new(UpstreamHopWriter::new("path-loss", "42", "tap"));
        let hops_cell = MemHops::default();

        inject("path-loss", Stage::TaphubSend, Duration::from_secs(5));

        let producer = {
            let tx = h.tx.clone();
            let probe = Arc::clone(&h.probe);
            let payload = h.payload.clone();
            let writer = Arc::clone(&writer);
            let hops_cell = hops_cell.clone();
            tokio::spawn(async move {
                for i in 20..200 {
                    // taphub read a frame from the tap and handed it to the
                    // transport; the record says so either way.
                    writer.on_source_frame();
                    writer.on_send();
                    hops_cell.publish(&writer);

                    if !dropping("path-loss", Stage::TaphubSend)
                        && !pump_chunk(&chunk(i, &payload), &tx, Some(&probe)).await
                    {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(FRAME_MS)).await;
                }
            })
        };

        let event = h
            .next_stall(Duration::from_secs(3))
            .await
            .expect("a quiet socket must be detected");
        // Let taphub keep producing for a moment after the engine gave up, so
        // the two sides are provably in different states.
        tokio::time::sleep(Duration::from_millis(100)).await;
        producer.abort();

        let frames_read_during_injection = h.probe.socket_recv() - socket_before;
        assert_eq!(
            frames_read_during_injection, 0,
            "the injection must stop every frame before the socket"
        );

        let upstream = UpstreamHopSource::read(&hops_cell, "path-loss").await;
        let record = upstream.clone().expect("taphub was publishing");
        assert!(
            now_unix_ms().saturating_sub(record.send_unix_ms) < 1_000,
            "the record kept advancing while taphub was still sending"
        );
        assert!(
            record.frames_sent > 5,
            "taphub kept producing through the injection window: {} frames",
            record.frames_sent
        );
        assert!(
            record.send_unix_ms > record.tap_recv_unix_ms.saturating_sub(1),
            "the record carries both of taphub's hops"
        );

        let report = build(&h.probe, event, upstream);
        assert_eq!(
            report.verdict,
            Verdict::TaphubToEnginePath,
            "{}",
            report.verdict_reason
        );
        assert!(report.counters.buffer_inserted_frames > 0);
        assert_eq!(
            report.counters.socket_recv_frames, report.counters.buffer_inserted_frames,
            "nothing was read after the path broke"
        );
    }

    /// Nothing moved anywhere: the tap (or the cache) is the quiet end.
    #[tokio::test]
    async fn a_tap_that_stops_is_named_as_the_tap() {
        clear_injection();
        let mut h = Harness::new("tap-quiet");
        h.run(0, 20).await;

        let event = h.next_stall(Duration::from_secs(3)).await.expect("stall");
        let report = build(&h.probe, event, Some(hops(5_000, 5_000, "tap-quiet")));
        assert_eq!(
            report.verdict,
            Verdict::TapOrCache,
            "{}",
            report.verdict_reason
        );
    }

    /// A stall with no upstream record must say so rather than blame a hop it
    /// cannot see.
    #[tokio::test]
    async fn no_upstream_record_is_reported_as_unknown() {
        clear_injection();
        let mut h = Harness::new("no-upstream");
        h.run(0, 20).await;

        let event = h.next_stall(Duration::from_secs(3)).await.expect("stall");
        let report = build(&h.probe, event, None);
        assert_eq!(report.verdict, Verdict::Unknown);
        assert_eq!(
            report.hops[zako3_audio_probe::HOP_TAP_TO_TAPHUB].source,
            "unavailable"
        );
        assert!(
            report.verdict_reason.contains("no upstream hop record"),
            "{}",
            report.verdict_reason
        );
    }
}
