//! What an audio engine asks HQ for, and what it gets back.

use serde::{Deserialize, Serialize};

use crate::cache::AudioCacheItem;
use crate::AudioMetaResponse;

/// A sink's claim on one request.
///
/// Minted by whichever process will receive the UDP stream — an audio engine
/// for playback, the cache worker for a preload — *before* it calls HQ. That
/// ordering is what removes the arm-before-first-packet race without a
/// handshake: by the time HQ can reach the tap, the receiver already exists.
///
/// HQ validates and routes a ticket; it never creates one.
#[derive(Clone, Serialize, Deserialize)]
pub struct SinkTicket {
    /// Unguessable, because it is also `ae_proxy`'s routing key and its
    /// source-pinning key, in the clear on every datagram.
    pub request_id: uuid::Uuid,
    /// Authenticates the request's datagrams in both directions.
    pub encryption_key: [u8; 32],
}

impl std::fmt::Debug for SinkTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The key must not reach a log line, and `Debug` is the easiest way for
        // one to get there by accident.
        f.debug_struct("SinkTicket")
            .field("request_id", &self.request_id)
            .field("encryption_key", &"<redacted>")
            .finish()
    }
}

impl SinkTicket {
    /// Whether this looks like something a sink actually minted.
    ///
    /// Cheap and not a security boundary — the RPC port is already behind an
    /// admin token — but it catches a caller that forgot to mint and sent
    /// zeroes, which would otherwise fail much later as an authentication
    /// error on the UDP path and look like a network fault.
    pub fn looks_valid(&self) -> bool {
        self.request_id.get_version_num() == 4 && self.encryption_key != [0u8; 32]
    }
}

/// HQ's answer to an audio request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioDispatch {
    /// Already cached. Stream it over HTTP from the cache server; no tap is
    /// involved and the ticket goes unused.
    CacheHit {
        meta: AudioMetaResponse,
        item: AudioCacheItem,
    },

    /// A tap accepted the request and is streaming to the ticket's address now.
    Dispatched {
        meta: AudioMetaResponse,
        /// Where to commit the audio, if this request is cacheable at all.
        cache_item: Option<AudioCacheItem>,
    },

    /// This tap has not been migrated. Use the taphub path, unchanged.
    ///
    /// One flag, checked in one place, so the cutover is per tap and reversible
    /// without a deploy.
    Legacy,
}

/// HQ's answer to a metadata or preload request.
///
/// Carries the same `Legacy` escape as [`AudioDispatch`] so the per-tap cutover
/// is decided in one place for every request shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetaDispatch {
    Ready(AudioMetaResponse),
    Legacy,
}

/// How a tap's transfer ended, as reported back to HQ.
///
/// HQ dispatches requests but never sees a byte of audio, so without this it
/// knows only what it asked for, never what was delivered — which is precisely
/// the failure this architecture is meant to make visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamReport {
    Completed { frames_sent: u64 },
    Aborted { frames_sent: u64, reason: String },
    /// The tap could not reach the sink at all. Distinguished from `Aborted`
    /// because it indicts the network path rather than the tap.
    Undeliverable { reason: String },
}
