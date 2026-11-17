use std::time::Duration;
use zako3_audio_engine_audio::ringbuf::traits::consumer::Consumer;

use crate::types::TrackId;
use zako3_audio_engine_audio::{
    Mixer, create_opus_ringbuf_pair, create_ringbuf_pair, create_thread_mixer,
};

#[tokio::test]
async fn test_mixer_add_remove_source() {
    let (output_prod, mut output_cons) = create_opus_ringbuf_pair();
    // Drain the output so the mixer never blocks, and keep the consumer alive for
    // the whole test: a dropped consumer is what ends the mixer thread (see
    // `test_mixer_reports_dead_once_the_output_consumer_is_dropped`).
    let _drain = tokio::spawn(async move {
        loop {
            while Consumer::try_pop(&mut output_cons).is_some() {}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
    let mixer = create_thread_mixer(output_prod);

    let track_id = TrackId::from(1);
    let (_source_prod, source_cons) = create_ringbuf_pair();
    let (end_tx, _end_rx) = tokio::sync::mpsc::channel(16);

    // Add source
    mixer.add_source(track_id, source_cons, end_tx);

    // Verify source is added
    assert!(mixer.has_source(track_id).await);

    // Remove source
    mixer.remove_source(track_id);

    // Wait a bit for the command to be processed
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify source is removed
    assert!(!mixer.has_source(track_id).await);
}

/// The mixer thread exits when its output consumer is dropped: nothing can
/// carry its frames any more. That has to be observable from outside, because
/// the session uses it to refuse to queue audio nothing will play.
#[tokio::test]
async fn test_mixer_reports_dead_once_the_output_consumer_is_dropped() {
    let (output_prod, output_cons) = create_opus_ringbuf_pair();
    let mixer = create_thread_mixer(output_prod);
    let alive = mixer.alive();
    assert!(
        alive.load(std::sync::atomic::Ordering::SeqCst),
        "a fresh mixer is alive"
    );

    drop(output_cons);

    // The thread checks at the top of each loop; it sleeps 10ms when idle.
    for _ in 0..100 {
        if !alive.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the mixer kept reporting itself alive after its consumer was dropped");
}
