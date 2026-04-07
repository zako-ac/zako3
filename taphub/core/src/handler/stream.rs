use bytes::Bytes;
use protofish2::mani::transfer::recv::TransferReliableRecvStream;
use tokio::sync::{mpsc, oneshot, watch};

/// Bridge a reliable stream to an `mpsc::Receiver<Bytes>`.
/// Also returns a oneshot that fires `()` only when the stream ends naturally
/// via TransferEnd/TransferEndAck AND the Tap has not disconnected.
/// If the Tap disconnects mid-stream (disconnect_rx becomes `true`) the task
/// exits without firing done_tx, so `done_rx.await` returns `Err` — preventing
/// partial audio from being committed to cache.
pub(crate) fn bridge_rel(
    mut rel: TransferReliableRecvStream,
    mut disconnect_rx: watch::Receiver<bool>,
) -> (mpsc::Receiver<Bytes>, oneshot::Receiver<()>) {
    let (tx, rx) = mpsc::channel(100);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        // Already disconnected before the stream even started.
        if *disconnect_rx.borrow() {
            tracing::warn!(
                "Tap already disconnected before stream started; stream will not be cached"
            );
            return;
        }

        'outer: loop {
            tokio::select! {
                biased;
                // Disconnect takes priority — checked first each iteration.
                _ = disconnect_rx.changed() => {
                    tracing::warn!("Tap disconnected mid-stream; aborting and discarding stream");
                    break 'outer; // done_tx dropped → cache discards partial data
                }
                chunks_opt = rel.recv() => {
                    match chunks_opt {
                        Some(chunks) => {
                            for chunk in chunks {
                                if tx.send(chunk.content).await.is_err() {
                                    break 'outer;
                                }
                            }
                        }
                        None => {
                            // Stream ended. Only consider it a clean TransferEnd
                            // if the Tap is still connected at this point.
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
    });
    (rx, done_rx)
}
