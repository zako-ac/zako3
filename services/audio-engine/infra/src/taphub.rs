use async_trait::async_trait;
use opentelemetry::global;
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_audio_engine_audio::metrics;
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::taphub::TapHubService,
    types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest},
};
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

use std::sync::Arc;
use zako3_audio_engine_core::error::ZakoError;
use zako3_taphub_transport_client::TransportClient;

pub struct RealTapHubService {
    client: Arc<tokio::sync::Mutex<Option<Arc<TransportClient>>>>,
}

impl RealTapHubService {
    pub fn new(client: Arc<TransportClient>) -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(Some(client))),
        }
    }

    pub fn new_lazy(client: Arc<tokio::sync::Mutex<Option<Arc<TransportClient>>>>) -> Self {
        Self { client }
    }

    async fn get_client(&self) -> ZakoResult<Arc<TransportClient>> {
        self.client
            .lock()
            .await
            .clone()
            .ok_or_else(|| ZakoError::TapHub("taphub not yet connected".to_string()))
    }
}

#[async_trait]
impl TapHubService for RealTapHubService {
    #[instrument(skip_all, fields(tap_id = %request.tap_id.0))]
    async fn request_audio(&self, mut request: CachedAudioRequest) -> ZakoResult<AudioResponse> {
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut request.headers));

        let start = std::time::Instant::now();

        let client = self.get_client().await?;
        let result = client
            .request_audio(request)
            .await
            .map_err(ZakoError::TapHub);

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(
            duration_ms = duration.as_millis(),
            "Audio stream requested (real)"
        );

        result
    }

    #[instrument(skip(self), fields(tap_id = %request.tap_id.0))]
    async fn preload_audio(&self, mut request: CachedAudioRequest) -> ZakoResult<AudioMetaResponse> {
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut request.headers));

        let start = std::time::Instant::now();

        let client = self.get_client().await?;
        let result = client
            .preload_audio(request)
            .await
            .map_err(ZakoError::TapHub);

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(
            duration_ms = duration.as_millis(),
            "Preload requested (real)"
        );

        result
    }

    #[instrument(skip(self), fields(tap_id = %request.tap_id.0))]
    async fn request_audio_meta(&self, mut request: AudioRequest) -> ZakoResult<AudioMetaResponse> {
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut request.headers));

        let start = std::time::Instant::now();

        let client = self.get_client().await?;
        let result = client
            .request_audio_meta(request)
            .await
            .map_err(ZakoError::TapHub);

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(
            duration_ms = duration.as_millis(),
            "Audio meta requested (real)"
        );

        result
    }
}

pub struct StubTapHubService;

#[async_trait]
impl TapHubService for StubTapHubService {
    #[instrument(skip_all, fields(tap_id = %request.tap_id.0))]
    async fn request_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioResponse> {
        let start = std::time::Instant::now();

        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let is_sine = request.audio_request.to_string().contains("sine");

        tokio::spawn(async move {
            let mut phase: f32 = 0.0;
            let sample_rate = 48000.0;
            let frequency = 440.0;
            let chunk_size = 960; // 20ms at 48kHz

            let mut interval = tokio::time::interval(std::time::Duration::from_millis(20));

            loop {
                interval.tick().await;

                let mut chunk = Vec::with_capacity(chunk_size * 2);
                for _ in 0..chunk_size {
                    let sample = if is_sine {
                        (phase * std::f32::consts::TAU).sin() * 10000.0
                    } else {
                        0.0
                    };
                    chunk.push(sample);
                    chunk.push(sample);

                    if is_sine {
                        phase += frequency / sample_rate;
                        if phase > 1.0 {
                            phase -= 1.0;
                        }
                    }
                }

                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
        });

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(duration_ms = duration.as_millis(), "Audio stream requested");

        Ok(AudioResponse {
            cache_key: Some(request.cache_key),
            metadatas: vec![AudioMetadata::Title("Dumym Title".to_string())],
            stream: rx,
        })
    }

    #[instrument(skip(self, _request))]
    async fn preload_audio(&self, _request: CachedAudioRequest) -> ZakoResult<AudioMetaResponse> {
        let start = std::time::Instant::now();

        let duration = start.elapsed();
        metrics::record_taphub_request_duration(duration.as_secs_f64());

        tracing::debug!(duration_ms = duration.as_millis(), "Preload requested");

        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    }

    #[instrument(skip(self, _request))]
    async fn request_audio_meta(&self, _request: AudioRequest) -> ZakoResult<AudioMetaResponse> {
        let start = std::time::Instant::now();

        let result = AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
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
    #[instrument(skip_all, fields(tap_id = %request.tap_id.0))]
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

    #[instrument(skip_all, fields(tap_id = %request.tap_id.0))]
    async fn preload_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioMetaResponse> {
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

    #[instrument(skip_all, fields(tap_id = %request.tap_id.0))]
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
