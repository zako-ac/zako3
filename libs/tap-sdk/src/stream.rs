use bytes::Bytes;
use protofish2::Timestamp;
use tokio::sync::mpsc;

/// Opaque handle for pushing encoded Opus frames to the Hub.
///
/// Dropping this sender signals end-of-stream to the Hub.
pub struct AudioStreamSender {
    pub(crate) tx: mpsc::Sender<(Timestamp, Bytes)>,
}

impl AudioStreamSender {
    /// Send a single Opus frame with an explicit timestamp (milliseconds).
    ///
    /// Returns `false` if the Hub has closed the connection and frames are no
    /// longer being consumed; the caller should stop sending.
    pub async fn send_frame(&self, ts: Timestamp, data: Bytes) -> bool {
        self.tx.send((ts, data)).await.is_ok()
    }

    /// Convenience wrapper: computes `Timestamp(frame_index * 20)`.
    ///
    /// Assumes standard Opus frames: 48 kHz sample rate, 960 samples per frame,
    /// giving 20 ms per frame.
    pub async fn send_opus_frame(&self, frame_index: u64, data: Bytes) -> bool {
        self.send_frame(Timestamp(frame_index * 20), data).await
    }
}
