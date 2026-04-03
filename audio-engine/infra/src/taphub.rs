use std::io::Cursor;

use async_trait::async_trait;
use tracing::instrument;
use zako3_audio_engine_audio::metrics;
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::taphub::TapHubService,
    types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest},
};
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

pub struct StubTapHubService;

static SPEAKY_DATA: &[u8] = include_bytes!("../good.mp3");
static SINE_DATA: &[u8] = include_bytes!("../sine.mp3");

#[async_trait]
impl TapHubService for StubTapHubService {
    #[instrument(skip_all, fields(tap_name = %request.tap_name))]
    async fn request_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioResponse> {
        let start = std::time::Instant::now();

        let cursor = if request.audio_request.to_string().contains("sine") {
            Cursor::new(SINE_DATA.to_vec())
        } else {
            Cursor::new(SPEAKY_DATA.to_vec())
        };

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(duration_ms = duration.as_millis(), "Audio stream requested");

        Ok(AudioResponse {
            cache_key: Some(request.cache_key),
            metadatas: vec![AudioMetadata::Title("Dumym Title".to_string())],
            stream: Box::new(cursor),
        })
    }

    #[instrument(skip(self, _request))]
    async fn preload_audio(&self, _request: CachedAudioRequest) -> ZakoResult<()> {
        let start = std::time::Instant::now();

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(duration_ms = duration.as_millis(), "Preload requested");

        Ok(())
    }

    #[instrument(skip(self), fields(tap_name = %_request.tap_name))]
    async fn request_audio_meta(&self, _request: AudioRequest) -> ZakoResult<AudioMetaResponse> {
        let start = std::time::Instant::now();

        let result = AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
        };

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(duration_ms = duration.as_millis(), "Audio meta requested");

        Ok(result)
    }
}

pub struct InstrumentedTapHubService<T: TapHubService> {
    inner: T,
}

impl<T: TapHubService> InstrumentedTapHubService<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<T: TapHubService> TapHubService for InstrumentedTapHubService<T> {
    #[instrument(skip_all, fields(tap_name = %request.tap_name))]
    async fn request_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioResponse> {
        let start = std::time::Instant::now();

        let result = self.inner.request_audio(request).await;

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        match &result {
            Ok(_) => {
                tracing::debug!(duration_ms = duration.as_millis(), "Audio stream fetched");
            }
            Err(e) => {
                tracing::error!(duration_ms = duration.as_millis(), error = %e, "Audio stream request failed");
                metrics::record_taphub_error("request_audio");
            }
        }

        result
    }

    #[instrument(skip_all, fields(tap_name = %request.tap_name))]
    async fn preload_audio(&self, request: CachedAudioRequest) -> ZakoResult<()> {
        let start = std::time::Instant::now();

        let result = self.inner.preload_audio(request).await;

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        match &result {
            Ok(_) => {
                tracing::debug!(duration_ms = duration.as_millis(), "Preload completed");
            }
            Err(e) => {
                tracing::error!(duration_ms = duration.as_millis(), error = %e, "Preload failed");
                metrics::record_taphub_error("preload_audio");
            }
        }

        result
    }

    #[instrument(skip_all, fields(tap_name = %request.tap_name))]
    async fn request_audio_meta(&self, request: AudioRequest) -> ZakoResult<AudioMetaResponse> {
        let start = std::time::Instant::now();

        let result = self.inner.request_audio_meta(request).await;

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        match &result {
            Ok(_) => {
                tracing::debug!(duration_ms = duration.as_millis(), "Audio meta fetched");
            }
            Err(e) => {
                tracing::error!(duration_ms = duration.as_millis(), error = %e, "Audio meta request failed");
                metrics::record_taphub_error("request_audio_meta");
            }
        }

        result
    }
}
