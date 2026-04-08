use async_trait::async_trait;
use bytes::Bytes;
use protofish2::Timestamp;
use tokio::sync::mpsc;
use zako3_taphub_transport_server::TapHubBridgeHandler;
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequest,
    CachedAudioRequest,
};

use crate::app::App;

#[async_trait]
impl TapHubBridgeHandler for App {
    async fn handle_request_audio(
        &self,
        request: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
        let (tx, rx) = mpsc::channel(1000);
        let is_sine = request.audio_request.to_string().contains("sine");

        tokio::spawn(async move {
            let mut phase: f32 = 0.0;
            let sample_rate = 48000.0;
            let frequency = 440.0;
            let chunk_size = 960; // 20ms at 48kHz

            let mut interval = tokio::time::interval(std::time::Duration::from_millis(20));
            let mut frame_count: u64 = 0;

            let mut encoder = opus::Encoder::new(
                sample_rate as u32,
                opus::Channels::Stereo,
                opus::Application::Audio,
            )
            .expect("failed to create opus encoder");

            loop {
                interval.tick().await;

                let mut chunk = Vec::with_capacity(chunk_size * 2);
                for _ in 0..chunk_size {
                    let sample = if is_sine {
                        (phase * std::f32::consts::TAU).sin() * 10000.0
                    } else {
                        0.0
                    };
                    let sample_i16 = sample as i16;
                    chunk.push(sample_i16); // Left
                    chunk.push(sample_i16); // Right

                    if is_sine {
                        phase += frequency / sample_rate;
                        if phase > 1.0 {
                            phase -= 1.0;
                        }
                    }
                }

                let mut out_opus = vec![0u8; 4000];
                match encoder.encode(&chunk, &mut out_opus) {
                    Ok(len) => {
                        let bytes = Bytes::copy_from_slice(&out_opus[..len]);
                        let ts = Timestamp(frame_count * 20);
                        frame_count += 1;
                        if tx.send((ts, bytes)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Opus encode error: {:?}", e);
                        break;
                    }
                }
            }
        });

        let meta = AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
        };

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
            base_volume: 1.0,
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
            base_volume: 1.0,
        })
    }
}
