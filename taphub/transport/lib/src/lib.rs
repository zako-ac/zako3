use serde::{Deserialize, Serialize};
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest, AudioMetadata, AudioCachePolicy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TapHubRequest {
    RequestAudio(CachedAudioRequest),
    PreloadAudio(CachedAudioRequest),
    RequestAudioMeta(AudioRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TapHubResponse {
    AudioReady(AudioMetaResponse),
    MetaReady(AudioMetaResponse),
    Error(String),
}
