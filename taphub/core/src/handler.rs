use async_trait::async_trait;
use bytes::Bytes;
use protofish2::Timestamp;
use tokio::sync::mpsc;
use zako3_taphub_transport_server::TapHubBridgeHandler;
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequest,
    CachedAudioRequest,
};

use crate::hub::TapHub;

#[async_trait]
impl TapHubBridgeHandler for TapHub {
    async fn handle_request_audio(
        &self,
        request: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
        let (tx, rx) = mpsc::channel(100);

        let states = &self
            .state_service
            .get_tap_states(&request.tap_id)
            .await
            .map_err(|e| format!("Failed to get tap states: {}", e))?;

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(states)
            .ok_or_else(|| "No available connections for this tap".to_string())?;

        let (succ, rel, mut unrel) = self
            .zf_hub
            .request_audio(
                request.tap_id.clone(),
                connection_id,
                request.audio_request,
                Default::default(),
            )
            .await
            .map_err(|e| format!("Failed to request audio from hub: {}", e))?;

        // TODO: cache rel

        let meta = AudioMetaResponse {
            metadatas: succ.metadatas,
            cache_key: succ.cache,
        };

        tokio::spawn(async move {
            while let Some(chunk) = unrel.recv().await {
                if let Err(e) = tx.send((chunk.timestamp, chunk.content)).await {
                    tracing::warn!(%e, "Failed to send audio chunk to channel");
                    return;
                }
            }
        });

        Ok((meta, rx))
    }

    async fn handle_preload_audio(
        &self,
        _req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
        })
    }

    async fn handle_request_audio_meta(
        &self,
        _req: AudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
        })
    }
}
