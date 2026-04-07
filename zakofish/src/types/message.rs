use serde::{Deserialize, Serialize};
use zako3_types::{AudioCachePolicy, AudioMetadata, AudioRequestString, hq::TapId};

use crate::types::HubRejectReasonType;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum TapToHubMessage {
    ClientHello(TapClientHello),
    AudioRequestSuccess(AudioRequestSuccessMessage),
    AudioRequestFailure(AudioRequestFailureMessage),
    AudioMetadataSuccess(AudioMetadataSuccessMessage),
}
