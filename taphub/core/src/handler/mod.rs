use async_trait::async_trait;
use bytes::Bytes;
use protofish2::Timestamp;
use tokio::sync::mpsc;
use zako3_taphub_transport_server::TapHubBridgeHandler;
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest};

use crate::hub::TapHub;

mod audio_request;
mod cache;
mod meta;
mod permission;
mod preload;
mod stream;

#[async_trait]
impl TapHubBridgeHandler for TapHub {
    async fn handle_request_audio(
        &self,
        request: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
        audio_request::handle_request_audio_inner(self, request).await
    }

    async fn handle_preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        preload::handle_preload_audio_inner(self, req).await
    }

    async fn handle_request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        meta::handle_request_audio_meta_inner(self, req).await
    }
}
