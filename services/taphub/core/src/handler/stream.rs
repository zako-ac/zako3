use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};
use zakofish_taphub::RelChunkStream;

use crate::metrics;

/// Bridge a reliable stream to an `mpsc::Receiver<Bytes>`.
/// Also returns a oneshot that fires `()` only when the stream ends naturally
/// AND the Tap has not disconnected. If the Tap disconnects mid-stream
/// (`disconnect_rx` becomes `true`) the task exits without firing `done_tx`,
/// so `done_rx.await` returns `Err` — preventing partial audio from being
/// committed to cache.
pub(crate) fn bridge_rel(
    mut rel: RelChunkStream,
    mut disconnect_rx: watch::Receiver<bool>,
) -> (mpsc::Receiver<Bytes>, oneshot::Receiver<()>) {
    let (tx, rx) = mpsc::channel(100);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        metrics::metrics().active_streams.add(1, &[]);

        if *disconnect_rx.borrow() {
            tracing::warn!(
                "Tap already disconnected before stream started; stream will not be cached"
            );
            metrics::metrics().active_streams.add(-1, &[]);
            return;
        }

        'outer: loop {
            tokio::select! {
                biased;
                _ = disconnect_rx.changed() => {
                    tracing::warn!("Tap disconnected mid-stream; aborting and discarding stream");
                    break 'outer;
                }
                chunk_opt = rel.recv() => {
                    match chunk_opt {
                        Some(chunk) => {
                            if tx.send(chunk).await.is_err() {
                                break 'outer;
                            }
                        }
                        None => {
                            if !*disconnect_rx.borrow() {
                                let _ = done_tx.send(());
                            } else {
                                tracing::warn!("Stream ended but Tap is already disconnected; discarding");
                            }
                            break 'outer;
                        }
                    }
                }
            }
        }

        metrics::metrics().active_streams.add(-1, &[]);
    });
    (rx, done_rx)
}
