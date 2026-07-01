use bytes::Bytes;
use std::sync::{Arc, OnceLock};
use zakofish::{Timestamp, TransferMode};
use tokio::sync::mpsc;

/// Opaque handle for pushing encoded Opus frames to the Hub.
///
/// Dropping this sender signals end-of-stream to the Hub.
pub struct AudioStreamSender {
    pub(crate) tx: mpsc::Sender<(Timestamp, Bytes)>,
    pub(crate) transfer_mode: Arc<OnceLock<TransferMode>>,
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

    /// Switch this stream to UnreliableOnly transfer mode.
    ///
    /// The hub will receive frames over the unreliable path only — no reliable
    /// retransmission, no caching, no end-to-end backpressure. Suitable for
    /// live/ephemeral audio where dropped frames are preferable to latency.
    ///
    /// Must be called before the handler returns; later calls are ignored.
    pub fn unreliable_only(&self) {
        let _ = self.transfer_mode.set(TransferMode::UnreliableOnly);
    }
}
