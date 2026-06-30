use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::mpsc;
use zako3_taphub_transport_server::{TapHubBridgeHandler, Timestamp};
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest, TapHubError};

use crate::hub::TapHub;

mod audio_request;
mod cache;
mod invalidate_cache;
mod meta;
mod permission;
mod preload;
mod stream;
pub(crate) mod tap_lookup;

#[async_trait]
impl TapHubBridgeHandler for TapHub {
    async fn handle_request_audio(
        &self,
        request: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), TapHubError> {
        audio_request::handle_request_audio_inner(self, request).await
    }

    async fn handle_preload_audio(
        &self,
        req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError> {
        preload::handle_preload_audio_inner(self, req).await
    }

    async fn handle_request_audio_meta(
        &self,
        req: AudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError> {
        meta::handle_request_audio_meta_inner(self, req).await
    }

    async fn handle_invalidate_cache(
        &self,
        req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<(), TapHubError> {
        invalidate_cache::handle_invalidate_cache_inner(self, req).await
    }
}
