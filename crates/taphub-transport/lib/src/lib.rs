use serde::{Deserialize, Serialize};
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest, TapHubError};

/// Audio frame timestamp in milliseconds.
///
/// protofish3 xfer chunks are opaque and carry no timestamp (unlike protofish2),
/// so the timestamp is carried in-band: the server prefixes each chunk with the
/// 8 big-endian bytes of this value via [`encode_chunk`], and the client strips
/// it back out via [`parse_chunk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub u64);

/// Prefix `data` with the 8-byte big-endian timestamp.
pub fn encode_chunk(ts: Timestamp, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(&ts.0.to_be_bytes());
    buf.extend_from_slice(data);
    buf
}

/// Split a chunk produced by [`encode_chunk`] back into its timestamp and body.
/// Returns `None` if the buffer is too short to contain the prefix.
pub fn parse_chunk(buf: &[u8]) -> Option<(Timestamp, &[u8])> {
    if buf.len() < 8 {
        return None;
    }
    let ts_bytes: [u8; 8] = buf[..8].try_into().expect("len checked");
    Some((Timestamp(u64::from_be_bytes(ts_bytes)), &buf[8..]))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TapHubRequest {
    RequestAudio(CachedAudioRequest),
    PreloadAudio(CachedAudioRequest),
    RequestAudioMeta(AudioRequest),
    InvalidateCache(CachedAudioRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TapHubResponse {
    AudioReady(AudioMetaResponse),
    MetaReady(AudioMetaResponse),
    Error(TapHubError),
    InvalidateCacheOk,
}
