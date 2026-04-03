use ringbuf::traits::Consumer;
use std::time::Duration;

use crate::types::TrackId;
use zako3_audio_engine_audio::{
    Mixer, create_opus_ringbuf_pair, create_ringbuf_pair, create_thread_mixer,
};

#[tokio::test]
async fn test_mixer_add_remove_source() {
    let (output_prod, mut output_cons) = create_opus_ringbuf_pair();
    // Drain output to prevent blocking the mixer thread
    tokio::spawn(async move { while let Some(_) = output_cons.try_pop() {} });
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
