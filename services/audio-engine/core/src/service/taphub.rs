use std::sync::Arc;

use crate::{
    error::ZakoResult,
    types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest},
};
use async_trait::async_trait;
use mockall::automock;

pub type ArcTapHubService = Arc<dyn TapHubService>;

#[automock]
#[async_trait]
pub trait TapHubService: Send + Sync + 'static {
    async fn request_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioResponse>;
    async fn preload_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioMetaResponse>;
    async fn request_audio_meta(&self, request: AudioRequest) -> ZakoResult<AudioMetaResponse>;
}
