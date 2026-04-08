#![allow(unused_variables)]
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

#[cfg(feature = "telemetry")]
use std::sync::OnceLock;

#[cfg(feature = "telemetry")]
use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Histogram, ObservableGauge, UpDownCounter},
};

pub struct AudioMetrics {
    pub mixer_underruns: AtomicU64,
    pub mixer_loops: AtomicU64,
    pub decoder_stalls: AtomicU64,
    pub stream_underruns: AtomicU64,
    pub mixer_buffer_depth: AtomicI64,
}

impl AudioMetrics {
    fn new() -> Self {
        Self {
            mixer_underruns: AtomicU64::new(0),
            mixer_loops: AtomicU64::new(0),
            decoder_stalls: AtomicU64::new(0),
            stream_underruns: AtomicU64::new(0),
            mixer_buffer_depth: AtomicI64::new(0),
        }
    }
}

pub static AUDIO_METRICS: std::sync::LazyLock<AudioMetrics> =
    std::sync::LazyLock::new(AudioMetrics::new);

#[cfg(feature = "telemetry")]
struct AudioOtelMetrics {
    mixer_underruns: Counter<u64>,
    decoder_stalls: Counter<u64>,
    stream_underruns: Counter<u64>,
    mixer_active_sources: UpDownCounter<i64>,
    mixer_processing_duration: Histogram<f64>,
    decode_errors: Counter<u64>,
    session_active: UpDownCounter<i64>,
    track_lifecycle: Counter<u64>,
    preload: Counter<u64>,
    taphub_request_duration: Histogram<f64>,
    taphub_errors: Counter<u64>,
    // Keep alive so the callback is not unregistered
    _mixer_buffer_depth_gauge: ObservableGauge<i64>,
}

#[cfg(feature = "telemetry")]
static OTEL_METRICS: OnceLock<AudioOtelMetrics> = OnceLock::new();

#[cfg(feature = "telemetry")]
fn otel() -> &'static AudioOtelMetrics {
    OTEL_METRICS.get_or_init(|| {
        let meter = global::meter("audio-engine");

        let buffer_depth_gauge = meter
            .i64_observable_gauge("audio_mixer_buffer_depth_samples")
            .with_description("Current available samples in the mixer buffer")
            .with_callback(|observer| {
                observer.observe(
                    AUDIO_METRICS.mixer_buffer_depth.load(Ordering::Relaxed),
                    &[],
                );
            })
            .build();

        AudioOtelMetrics {
            mixer_underruns: meter
                .u64_counter("audio_mixer_underruns_total")
                .with_description("Total number of mixer underruns (starvation)")
                .build(),
            decoder_stalls: meter
                .u64_counter("audio_decoder_stalls_total")
                .with_description("Total number of decoder stalls (buffer full)")
                .build(),
            stream_underruns: meter
                .u64_counter("audio_stream_underruns_total")
                .with_description("Total number of output stream underruns")
                .build(),
            mixer_active_sources: meter
                .i64_up_down_counter("audio_mixer_active_sources")
                .with_description("Current number of audio sources being mixed")
                .build(),
            mixer_processing_duration: meter
                .f64_histogram("audio_mixer_processing_duration_seconds")
                .with_description("Time taken for a single mixer loop iteration")
                .with_unit("s")
                .with_boundaries(vec![
                    0.001, 0.005, 0.010, 0.015, 0.020, 0.025, 0.030, 0.050, 0.100,
                ])
                .build(),
            decode_errors: meter
                .u64_counter("audio_decode_errors_total")
                .with_description("Total number of audio decoding errors")
                .build(),
            session_active: meter
                .i64_up_down_counter("audio_session_active_total")
                .with_description("Number of active audio sessions")
                .build(),
            track_lifecycle: meter
                .u64_counter("audio_track_lifecycle_total")
                .with_description("Track lifecycle events")
                .build(),
            preload: meter
                .u64_counter("audio_preload_total")
                .with_description("Audio preload attempts")
                .build(),
            taphub_request_duration: meter
                .f64_histogram("taphub_request_duration_seconds")
                .with_description("Latency of TapHub API requests")
                .with_unit("s")
                .with_boundaries(vec![
                    0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.0, 2.5, 5.0, 10.0,
                ])
                .build(),
            taphub_errors: meter
                .u64_counter("taphub_errors_total")
                .with_description("Total number of TapHub request errors")
                .build(),
            _mixer_buffer_depth_gauge: buffer_depth_gauge,
        }
    })
}

pub fn record_mixer_underrun() {
    AUDIO_METRICS
        .mixer_underruns
        .fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "telemetry")]
    otel().mixer_underruns.add(1, &[]);
}

pub fn record_mixer_buffer_depth(depth: usize) {
    AUDIO_METRICS
        .mixer_buffer_depth
        .store(depth as i64, Ordering::Relaxed);
}

pub fn record_decoder_stall() {
    AUDIO_METRICS.decoder_stalls.fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "telemetry")]
    otel().decoder_stalls.add(1, &[]);
}

pub fn record_stream_underrun() {
    AUDIO_METRICS
        .stream_underruns
        .fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "telemetry")]
    otel().stream_underruns.add(1, &[]);
}

pub fn record_mixer_active_sources(_count: i64) {}

pub fn inc_mixer_active_sources() {
    #[cfg(feature = "telemetry")]
    otel().mixer_active_sources.add(1, &[]);
}

pub fn dec_mixer_active_sources() {
    #[cfg(feature = "telemetry")]
    otel().mixer_active_sources.add(-1, &[]);
}

pub fn record_mixer_processing_duration(duration_secs: f64) {
    #[cfg(feature = "telemetry")]
    otel().mixer_processing_duration.record(duration_secs, &[]);
}

pub fn record_decode_error(error_type: &str) {
    #[cfg(feature = "telemetry")]
    otel()
        .decode_errors
        .add(1, &[KeyValue::new("error_type", error_type.to_string())]);
}

pub fn inc_session_active() {
    #[cfg(feature = "telemetry")]
    otel().session_active.add(1, &[]);
}

pub fn dec_session_active() {
    #[cfg(feature = "telemetry")]
    otel().session_active.add(-1, &[]);
}

pub fn record_track_lifecycle(event: &str, queue_name: &str) {
    #[cfg(feature = "telemetry")]
    otel().track_lifecycle.add(
        1,
        &[
            KeyValue::new("event", event.to_string()),
            KeyValue::new("queue_name", queue_name.to_string()),
        ],
    );
}

pub fn record_preload(result: &str) {
    #[cfg(feature = "telemetry")]
    otel()
        .preload
        .add(1, &[KeyValue::new("result", result.to_string())]);
}

pub fn record_taphub_request_duration(duration_secs: f64) {
    #[cfg(feature = "telemetry")]
    otel().taphub_request_duration.record(duration_secs, &[]);
}

pub fn record_taphub_error(endpoint: &str) {
    #[cfg(feature = "telemetry")]
    otel()
        .taphub_errors
        .add(1, &[KeyValue::new("endpoint", endpoint.to_string())]);
}
