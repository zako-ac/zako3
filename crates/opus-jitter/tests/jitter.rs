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

/// The hard cap still exists for a sender that cannot be paced: a UDP tap has
/// no back channel, so the only way to stay bounded is to cut it off. The
/// protofish3 version had no cap at all and grew its `BTreeMap` without limit.
#[tokio::test]
async fn the_hard_cap_still_bounds_a_sender_that_cannot_be_paced() {
    let frames = encode_frames(1);
    let (tx, rx) = mpsc::channel(4096);

    // A lead window far wider than the cap, standing in for a source with no
    // back channel at all: nothing but the cap can stop it.
    let cfg = JitterConfig {
        max_buffered_frames: 32,
        max_lead_ms: 10_000,
        ..cfg()
    };
    for i in 0..300u64 {
        let _ = tx.try_send(TimedFrame { ts_ms: i * FRAME_MS, payload: frames[0].clone() });
    }
    drop(tx);

    let mut jb = OpusJitterBuffer::new(rx, cfg).unwrap();
    let mut count = 0;
    loop {
        match jb.yield_pcm().await {
            Ok(Some(_)) => count += 1,
            Ok(None) => break,
            Err(e) => panic!("playback failed: {e}"),
        }
    }
    assert_eq!(count, 32, "only the capped number of frames can be played");
    assert!(
        jb.dropped_frames() > 0,
        "frames beyond the cap should be dropped rather than buffered"
    );
}

/// A stall that skips frames must not leave them behind. They can never be
/// played — the play head has passed them — and left in the map they would both
/// count against the lead window and, once the channel ran dry, let the next
/// recovery rewind the head onto audio that had already gone out.
#[tokio::test]
async fn a_stall_never_rewinds_onto_the_frames_it_skipped() {
    let frames = encode_frames(6);
    let (tx, rx) = mpsc::channel(16);
    // 0 ms, then a hole, then 60/80/100 ms. The hole is smaller than the
    // playout budget, so this is a stall rather than a concealment.
    for i in [0usize, 3, 4, 5] {
        tx.try_send(TimedFrame {
            ts_ms: i as u64 * FRAME_MS,
            payload: frames[i].clone(),
        })
        .expect("send");
    }
    let _held = tx;

    let mut jb = OpusJitterBuffer::new(rx, cfg()).unwrap();

    let mut played = 0;
    let err = loop {
        match jb.yield_pcm().await {
            Ok(Some(_)) => played += 1,
            Ok(None) => break None,
            Err(e) => break Some(e),
        }
    };

    // One frame before the hole and one after it; the three the jump passed over
    // are gone, so the next stall ends what this buffer has.
    assert_eq!(played, 2, "the skipped frames must not be replayed");
    assert!(matches!(err, Some(JitterError::Stalled(_))));
}

/// A sender that runs ahead of real time is now *held back* rather than
/// decimated: the frames it could not hand over stay in the channel, and every
/// hop upstream waits on them. This is the bug the cap was papering over — a
/// tap decoding from a file used to fill the buffer and have two thirds of the
/// track thrown away.
#[tokio::test]
async fn a_sender_running_ahead_is_paced_instead_of_decimated() {
    const FRAMES: u64 = 1000;
    let frames = encode_frames(1);
    let (tx, rx) = mpsc::channel(64);

    // A source that produces as fast as it is allowed to, and keeps the
    // channel open afterwards.
    let sender = tokio::spawn(async move {
        for i in 0..FRAMES {
            let frame = TimedFrame { ts_ms: i * FRAME_MS, payload: frames[0].clone() };
            if tx.send(frame).await.is_err() {
                return;
            }
        }
        // Hold the sender open: this is a tap mid-track, not one that ended.
        std::future::pending::<()>().await;
    });

    let mut jb = OpusJitterBuffer::new(rx, cfg()).unwrap();
    let mut played = 0;
    loop {
        match jb.yield_pcm().await {
            Ok(Some(_)) => {
                played += 1;
                if played == FRAMES {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => panic!("playback failed after {played} frames: {e}"),
        }
    }

    assert_eq!(played, FRAMES, "every frame the sender produced must be played");
    assert_eq!(jb.dropped_frames(), 0, "pacing leaves nothing to drop");
    sender.abort();
}

/// Nothing plays until the pre-roll is held, so the playout delay is a buffer
/// rather than a per-frame allowance.
#[tokio::test]
async fn playback_waits_for_the_pre_roll() {
    let frames = encode_frames(6);
    let (tx, rx) = mpsc::channel(16);
    // Two frames is 40 ms: well short of the 200 ms pre-roll.
    for i in 0..2u64 {
        tx.try_send(TimedFrame { ts_ms: i * FRAME_MS, payload: frames[i as usize].clone() })
            .unwrap();
    }

    let cfg = JitterConfig {
        pre_roll_ms: 200,
        stall_timeout: Duration::from_secs(30),
        ..cfg()
    };
    let mut jb = OpusJitterBuffer::new(rx, cfg).unwrap();

    let early = tokio::time::timeout(Duration::from_millis(50), jb.yield_pcm()).await;
    assert!(
        early.is_err(),
        "playback must not start before the pre-roll is buffered"
    );

    // The rest of the stream arrives, and then plays out in full.
    for i in 2..6u64 {
        tx.send(TimedFrame { ts_ms: i * FRAME_MS, payload: frames[i as usize].clone() })
            .await
            .unwrap();
    }
    drop(tx);

    let mut played = 0;
    loop {
        match jb.yield_pcm().await {
            Ok(Some(_)) => played += 1,
            Ok(None) => break,
            Err(e) => panic!("playback failed: {e}"),
        }
    }
    assert_eq!(played, 6);
}

/// With the pre-roll disabled the first frame goes out as soon as it arrives,
/// which is the behaviour everything had before.
#[tokio::test]
async fn without_a_pre_roll_playback_starts_immediately() {
    let frames = encode_frames(1);
    let (tx, rx) = mpsc::channel(16);
    tx.try_send(TimedFrame { ts_ms: 0, payload: frames[0].clone() }).unwrap();

    let cfg = JitterConfig { pre_roll_ms: 0, ..cfg() };
    let mut jb = OpusJitterBuffer::new(rx, cfg).unwrap();
    let held = tx;

    let first = tokio::time::timeout(Duration::from_millis(50), jb.yield_pcm()).await;
    assert!(first.is_ok(), "the first frame should play without waiting");
    drop(held);
}

/// A gap that outlasts the stall timeout is not the end of the stream while
/// frames are still buffered behind it: playback resumes at the newest frame
/// rather than dropping the rest of the track.
#[tokio::test]
async fn a_stall_with_audio_still_buffered_resumes() {
    let frames = encode_frames(6);
    let (tx, rx) = mpsc::channel(16);
    // A hole between 40 ms and 100 ms that will never fill, with audio past it.
    for i in [0usize, 1, 5] {
        tx.try_send(TimedFrame { ts_ms: i as u64 * FRAME_MS, payload: frames[i].clone() })
            .unwrap();
    }
    let _held = tx;

    let mut jb = OpusJitterBuffer::new(rx, cfg()).unwrap();

    let mut played = 0;
    let err = loop {
        match jb.yield_pcm().await {
            Ok(Some(_)) => played += 1,
            Ok(None) => break None,
            Err(e) => break Some(e),
        }
    };

    assert_eq!(played, 3, "the frames past the gap must still be played");
    assert!(
        matches!(err, Some(JitterError::Stalled(_))),
        "only once nothing is left should the stall surface"
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
