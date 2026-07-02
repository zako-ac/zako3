//! Transport-level value types owned by zakofish.
//!
//! These used to be re-exported from protofish2. zakofish now defines them
//! itself so the public `TapHandler` API does not leak a transport crate's
//! types. The pf3 sender maps [`TransferMode`] onto `protofish3::XferMode` in
//! [`crate::tap_pf3`].

/// Audio frame timestamp in milliseconds.
///
/// protofish3 xfer chunks are opaque and carry no timestamp, so it is carried
/// in-band: the sender prefixes each chunk with the 8 big-endian bytes of this
/// value (see [`crate::tap_streams::encode_pf3_chunk`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub u64);

/// How a tap wants to stream audio chunks to the hub.
///
/// `Dual` uses both the reliable and unreliable paths; `UnreliableOnly` skips
/// the reliable path (and its backpressure/caching on the hub side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferMode {
    #[default]
    Dual,
    UnreliableOnly,
}
