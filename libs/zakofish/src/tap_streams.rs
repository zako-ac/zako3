//! Transport-agnostic chunk recv wrappers.
//!
//! The hub returns `RelChunkStream` / `UnrelChunkStream` for a pf3 tap
//! connection. Internally:
//!
//! - **pf3 Dual**: separate reliable + unreliable streams are bridged
//!   independently into the two wrappers.
//! - **pf3 Unrel**: only the unreliable wrapper is produced.
//!
//! pf3 chunks carry no timestamp in the proto layer — the zakofish sender
//! prefixes each chunk with 8 big-endian bytes (`u64` milliseconds via
//! [`encode_pf3_chunk`]); the receive pumps strip and parse the prefix.

use bytes::Bytes;
use protofish3::XferMode;
use protofish3::xfer::{DualRecvXfer, RecvXfer, XferRecv};
use tokio::sync::{mpsc, oneshot};

use crate::types::Timestamp;

const CHANNEL_BUFFER: usize = 100;

pub struct RelChunkStream {
    rx: mpsc::Receiver<Bytes>,
}

impl RelChunkStream {
    pub async fn recv(&mut self) -> Option<Bytes> {
        self.rx.recv().await
    }
}

pub struct UnrelChunkStream {
    rx: mpsc::Receiver<(Timestamp, Bytes)>,
}

impl UnrelChunkStream {
    pub async fn recv(&mut self) -> Option<(Timestamp, Bytes)> {
        self.rx.recv().await
    }
}

/// Prefix `data` with an 8-byte big-endian timestamp. Used by the pf3 tap
/// sender path to embed the timestamp inside the otherwise-opaque pf3 xfer
/// chunk.
pub fn encode_pf3_chunk(ts: Timestamp, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(&ts.0.to_be_bytes());
    buf.extend_from_slice(data);
    buf
}

fn parse_pf3_chunk(data: Vec<u8>) -> Option<(Timestamp, Bytes)> {
    if data.len() < 8 {
        tracing::warn!(len = data.len(), "pf3 chunk smaller than 8 bytes; dropping");
        return None;
    }
    let ts_bytes: [u8; 8] = data[..8].try_into().expect("len checked");
    let ts = Timestamp(u64::from_be_bytes(ts_bytes));
    let body = Bytes::copy_from_slice(&data[8..]);
    Some((ts, body))
}

/// Drive a pf3 `ChanReceiver`: accept the inbound xfer, signal the negotiated
/// mode via `mode_tx`, and pump chunks into the wrapper streams. Each chunk
/// has its 8-byte timestamp prefix stripped (rel discards it; unrel parses it).
///
/// For `Dual`: both rel and unrel are populated.
/// For `Unrel` (single): only unrel is populated; rel closes immediately.
/// For `Rel` (single): only rel is populated; unrel closes immediately.
pub fn bridge_pf3_recv(
    chan_receiver: protofish3::ChanReceiver,
    mode_tx: oneshot::Sender<XferMode>,
) -> (RelChunkStream, UnrelChunkStream) {
    let (rel_tx, rel_rx) = mpsc::channel::<Bytes>(CHANNEL_BUFFER);
    let (unrel_tx, unrel_rx) = mpsc::channel::<(Timestamp, Bytes)>(CHANNEL_BUFFER);
    tokio::spawn(async move {
        let mut chan_receiver = chan_receiver;
        let xfer_recv = match chan_receiver.accept_xfer().await {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!(error = %e, "pf3 accept_xfer failed");
                return;
            }
        };
        match xfer_recv {
            XferRecv::Dual(mut dual) => {
                let _ = mode_tx.send(XferMode::Dual);
                pump_dual(&mut dual, rel_tx, unrel_tx).await;
            }
            XferRecv::Single(single) => {
                let mode = single.mode();
                let _ = mode_tx.send(mode);
                match mode {
                    XferMode::Unrel => {
                        drop(rel_tx);
                        pump_single_unrel(single, unrel_tx).await;
                    }
                    XferMode::Rel => {
                        drop(unrel_tx);
                        pump_single_rel(single, rel_tx).await;
                    }
                    XferMode::Dual => unreachable!("Dual handled above"),
                }
            }
        }
    });
    (RelChunkStream { rx: rel_rx }, UnrelChunkStream { rx: unrel_rx })
}

async fn pump_dual(
    dual: &mut DualRecvXfer<'_>,
    rel_tx: mpsc::Sender<Bytes>,
    unrel_tx: mpsc::Sender<(Timestamp, Bytes)>,
) {
    let mut rel_done = false;
    let mut unrel_done = false;
    while !rel_done || !unrel_done {
        tokio::select! {
            maybe = dual.rel.recv(), if !rel_done => match maybe {
                Some(data) => match parse_pf3_chunk(data) {
                    Some((_, body)) => {
                        if rel_tx.send(body).await.is_err() {
                            rel_done = true;
                        }
                    }
                    None => continue,
                },
                None => rel_done = true,
            },
            maybe = dual.unrel.recv(), if !unrel_done => match maybe {
                Some(data) => match parse_pf3_chunk(data) {
                    Some(parsed) => {
                        if unrel_tx.send(parsed).await.is_err() {
                            unrel_done = true;
                        }
                    }
                    None => continue,
                },
                None => unrel_done = true,
            },
        }
    }
}

async fn pump_single_unrel(mut single: RecvXfer<'_>, unrel_tx: mpsc::Sender<(Timestamp, Bytes)>) {
    while let Some(data) = single.recv().await {
        let Some(parsed) = parse_pf3_chunk(data) else {
            continue;
        };
        if unrel_tx.send(parsed).await.is_err() {
            return;
        }
    }
}

async fn pump_single_rel(mut single: RecvXfer<'_>, rel_tx: mpsc::Sender<Bytes>) {
    while let Some(data) = single.recv().await {
        let Some((_, body)) = parse_pf3_chunk(data) else {
            continue;
        };
        if rel_tx.send(body).await.is_err() {
            return;
        }
    }
}
