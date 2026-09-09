//! Feeds a protofish3 transfer into the shared jitter buffer.
//!
//! The buffer itself now lives in `zako3-opus-jitter`, so the protofish4 path
//! uses the same one. What is left here is unwrapping protofish3's framing: its
//! chunks are opaque, so the timestamp travels as an 8-byte prefix the server
//! adds by hand.

use protofish3::xfer::RecvXfer;
use tokio::sync::mpsc;
use zako3_opus_jitter::TimedFrame;
use zako3_taphub_transport_lib::parse_chunk;

/// Frames buffered between the transfer and the jitter buffer.
///
/// Small: the jitter buffer drains this eagerly and does its own bounding, so a
/// deep queue here would only hide backlog from the frame cap.
pub const CHANNEL_CAP: usize = 64;

/// Pump a transfer into `tx` until it ends.
///
/// Takes the transfer by reference rather than returning a receiver, because
/// `RecvXfer` borrows the channel receiver it came from and cannot outlive it —
/// so the pump has to run alongside the consumer rather than in a task of its
/// own.
pub async fn pump(xfer: &mut RecvXfer<'_>, tx: mpsc::Sender<TimedFrame>) {
    while let Some(data) = xfer.recv().await {
        let Some((ts, body)) = parse_chunk(&data) else {
            tracing::warn!(len = data.len(), "pf3 chunk shorter than its timestamp prefix");
            continue;
        };
        if tx
            .send(TimedFrame { ts_ms: ts.0, payload: body.to_vec() })
            .await
            .is_err()
        {
            break;
        }
    }
}
