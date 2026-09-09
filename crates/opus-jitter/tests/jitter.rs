//! Reordering, concealment and the memory bound.
//!
//! Real Opus frames throughout: a synthetic buffer would not exercise the
//! decoder, and packet-loss concealment is the one behaviour here that only the
//! codec can provide.

use std::time::Duration;

use tokio::sync::mpsc;
use zako3_opus_jitter::{JitterConfig, JitterError, OpusJitterBuffer, TimedFrame};

const FRAME_MS: u64 = 20;
const SAMPLES_PER_FRAME: usize = 960; // 20 ms at 48 kHz

/// Encode `n` frames of a quiet tone, so the decoder has something real to
/// chew on.
fn encode_frames(n: usize) -> Vec<Vec<u8>> {
    let mut enc = opus::Encoder::new(48_000, opus::Channels::Stereo, opus::Application::Audio)
        .expect("encoder");
    let mut phase = 0f32;
    (0..n)
        .map(|_| {
            let mut pcm = Vec::with_capacity(SAMPLES_PER_FRAME * 2);
            for _ in 0..SAMPLES_PER_FRAME {
                let s = (phase * std::f32::consts::TAU).sin() * 0.1;
                pcm.push(s);
                pcm.push(s);
                phase = (phase + 440.0 / 48_000.0) % 1.0;
            }
            enc.encode_vec_float(&pcm, 4000).expect("encode")
        })
        .collect()
}

fn cfg() -> JitterConfig {
    JitterConfig {
        stall_timeout: Duration::from_millis(200),
        ..Default::default()
    }
}

/// Feed frames in the given arrival order, then close the stream.
fn feed(order: Vec<usize>, frames: Vec<Vec<u8>>) -> mpsc::Receiver<TimedFrame> {
    let (tx, rx) = mpsc::channel(256);
    for i in order {
        tx.try_send(TimedFrame {
            ts_ms: i as u64 * FRAME_MS,
            payload: frames[i].clone(),
        })
        .expect("send");
    }
    drop(tx);
    rx
}

async fn drain(mut jb: OpusJitterBuffer) -> (usize, Result<(), JitterError>) {
    let mut count = 0;
    loop {
        match jb.yield_pcm().await {
            Ok(Some(pcm)) => {
                assert_eq!(pcm.len(), SAMPLES_PER_FRAME * 2, "one stereo frame");
                count += 1;
            }
            Ok(None) => return (count, Ok(())),
            Err(e) => return (count, Err(e)),
        }
    }
}

#[tokio::test]
async fn frames_in_order_come_out_in_order() {
    let frames = encode_frames(10);
    let jb = OpusJitterBuffer::new(feed((0..10).collect(), frames), cfg()).unwrap();
    let (count, res) = drain(jb).await;
    assert!(res.is_ok());
    assert_eq!(count, 10);
}

/// The reason the buffer exists: UDP delivers out of order, and playback must
/// not.
#[tokio::test]
async fn out_of_order_arrivals_are_reordered() {
    let frames = encode_frames(6);
    let jb = OpusJitterBuffer::new(feed(vec![0, 3, 1, 2, 5, 4], frames), cfg()).unwrap();
    let (count, res) = drain(jb).await;
    assert!(res.is_ok());
    assert_eq!(count, 6, "every frame should still be played");
}

/// A frame that never arrives is concealed by the codec rather than dropped, so
/// the stream stays the right length and the loss sounds like a smear rather
/// than a click.
#[tokio::test]
async fn a_missing_frame_is_concealed_not_skipped() {
    let frames = encode_frames(20);
    // Frame 5 never arrives.
    let order: Vec<usize> = (0..20).filter(|i| *i != 5).collect();
    let jb = OpusJitterBuffer::new(feed(order, frames), cfg()).unwrap();

    let (count, res) = drain(jb).await;
    assert!(res.is_ok());
    assert_eq!(count, 20, "the gap should be filled, not skipped over");
}

/// A retransmission that arrives after its slot was already concealed is
/// discarded — playing it now would move audio backwards.
#[tokio::test]
async fn a_late_retransmission_is_dropped_rather_than_played_out_of_order() {
    let frames = encode_frames(4);
    let (tx, rx) = mpsc::channel(16);
    for i in [0usize, 1, 2, 3] {
        tx.try_send(TimedFrame { ts_ms: i as u64 * FRAME_MS, payload: frames[i].clone() }).unwrap();
    }
    // Frame 0 again, long after it played.
    tx.try_send(TimedFrame { ts_ms: 0, payload: frames[0].clone() }).unwrap();
    drop(tx);

    let jb = OpusJitterBuffer::new(rx, cfg()).unwrap();
    let (count, res) = drain(jb).await;
    assert!(res.is_ok());
    assert_eq!(count, 4, "the duplicate must not add a frame");
}

#[tokio::test]
async fn a_stream_that_never_produces_anything_ends_cleanly() {
    let (tx, rx) = mpsc::channel::<TimedFrame>(1);
    drop(tx);
    let mut jb = OpusJitterBuffer::new(rx, cfg()).unwrap();
    assert!(jb.yield_pcm().await.unwrap().is_none());
}

/// A sender that goes silent mid-stream must not hang playback forever.
#[tokio::test]
async fn a_stalled_sender_is_reported() {
    let frames = encode_frames(2);
    let (tx, rx) = mpsc::channel(4);
    tx.try_send(TimedFrame { ts_ms: 0, payload: frames[0].clone() }).unwrap();
    // Hold the sender open and send nothing more.
    let _held = tx;

    let mut jb = OpusJitterBuffer::new(rx, cfg()).unwrap();
    let _ = jb.yield_pcm().await;
    let err = loop {
        match jb.yield_pcm().await {
            Ok(_) => continue,
            Err(e) => break e,
        }
    };
    assert!(matches!(err, JitterError::Stalled(_)));
}

/// The protofish3 version had an unbounded `BTreeMap`, so a tap decoding from a
/// file could grow it without limit. Bounding only the reliable stream
/// elsewhere would have moved the same problem here.
#[tokio::test]
async fn the_buffer_is_bounded_against_a_sender_running_ahead() {
    let frames = encode_frames(1);
    let (tx, rx) = mpsc::channel(4096);

    // Far more than the cap, all buffered before anything is consumed.
    for i in 0..300u64 {
        let _ = tx.try_send(TimedFrame { ts_ms: i * FRAME_MS, payload: frames[0].clone() });
    }
    drop(tx);

    let cfg = JitterConfig { max_buffered_frames: 32, ..cfg() };
    let mut jb = OpusJitterBuffer::new(rx, cfg).unwrap();

    // Pull one frame, which forces the buffer to fill first.
    let _ = jb.yield_pcm().await.unwrap();
    assert!(
        jb.dropped_frames() > 0,
        "frames beyond the cap should be dropped rather than buffered"
    );
}

/// Occupancy is reported back to the sender so it can pace itself; in
/// milliseconds rather than frames, because that is what a sender can act on.
#[tokio::test]
async fn occupancy_is_reported_in_milliseconds() {
    let frames = encode_frames(10);
    let (tx, rx) = mpsc::channel(64);
    for (i, frame) in frames.iter().enumerate() {
        tx.try_send(TimedFrame { ts_ms: i as u64 * FRAME_MS, payload: frame.clone() }).unwrap();
    }
    drop(tx);

    let mut jb = OpusJitterBuffer::new(rx, cfg()).unwrap();
    assert_eq!(jb.buffered_ms(), 0, "nothing is buffered before the first pull");

    let _ = jb.yield_pcm().await.unwrap();
    let buffered = jb.buffered_ms();
    assert!(
        (100..=200).contains(&buffered),
        "ten 20 ms frames minus what played: got {buffered}ms"
    );
}
