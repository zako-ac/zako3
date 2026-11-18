use serde::{Deserialize, Serialize};

use crate::types::model::{
    AudioCachePolicy, AudioMetadata, AudioRequestString, HubRejectReasonType, TapId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AttachedMetadata {
    #[serde(rename = "use_cached")]
    UseCached,
    #[serde(rename = "metadatas")]
    Metadatas(Vec<AudioMetadata>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRequestMessage {
    pub ars: AudioRequestString,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadataRequestMessage {
    pub ars: AudioRequestString,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRequestSuccessMessage {
    pub cache: AudioCachePolicy,
    pub duration_secs: Option<f32>,
    pub metadatas: AttachedMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadataSuccessMessage {
    pub metadatas: Vec<AudioMetadata>,
    pub cache: AudioCachePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRequestFailureMessage {
    pub reason: String,
    pub try_others: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapClientHello {
    pub tap_id: TapId,
    pub friendly_name: String,
    pub api_token: String,
    pub selection_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapServerReject {
    pub reason_type: HubRejectReasonType,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum HubToTapMessage {
    Accept,
    Reject(TapServerReject),
    AudioRequest(AudioRequestMessage),
    AudioMetadataRequest(AudioMetadataRequestMessage),
}

/// A tap telling the hub it is going away on purpose.
///
/// Sent on its own chan as soon as the tap has been asked to shut down, before
/// it stops serving. The hub's job with it is to take the connection out of
/// routing *now*, rather than waiting for the QUIC connection to notice a
/// process that has stopped reading — which is what a pod deletion looks like
/// from the network, and which takes as long as the idle timeout.
///
/// `in_flight` is advisory: the tap keeps serving what it already accepted, and
/// the hub's own in-flight requests are failed rather than dribbled out, so the
/// count is for logs and for deciding whether to wait at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapDraining {
    /// Requests this tap still has running.
    pub in_flight: u32,
    /// Unix milliseconds when the tap started draining. The hub subtracts this
    /// from its own clock when it acts on the message, so the recorded latency
    /// is the whole hop (tap → hub → out of the routing table) and not just the
    /// hub's local processing.
    pub started_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum TapToHubMessage {
    ClientHello(TapClientHello),
    AudioRequestSuccess(AudioRequestSuccessMessage),
    AudioRequestFailure(AudioRequestFailureMessage),
    AudioMetadataSuccess(AudioMetadataSuccessMessage),
    Draining(TapDraining),
}
