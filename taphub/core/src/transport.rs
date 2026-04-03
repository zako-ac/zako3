use async_trait::async_trait;
use bytes::Bytes;
use protofish2::Timestamp;
use tokio::sync::mpsc;
use zako3_taphub_transport_server::TapHubBridgeHandler;
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest};

use crate::app::App;

#[async_trait]
impl TapHubBridgeHandler for App {
    async fn handle_request_audio(
        &self,
        _req: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
        todo!("Implement request_audio business logic");
    }

    async fn handle_preload_audio(
        &self,
        _req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        todo!("Implement preload_audio business logic");
    }

    async fn handle_request_audio_meta(
        &self,
        _req: AudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        todo!("Implement request_audio_meta business logic");
    }
}
