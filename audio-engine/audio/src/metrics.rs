use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "telemetry")]
use lazy_static::lazy_static;

#[cfg(feature = "telemetry")]
use prometheus::{
    CounterVec, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, register_counter_vec,
    register_histogram, register_int_counter, register_int_gauge,
};

pub struct AudioMetrics {
    pub mixer_underruns: AtomicU64,
    pub mixer_loops: AtomicU64,
    pub decoder_stalls: AtomicU64,
    pub stream_underruns: AtomicU64,
}

impl AudioMetrics {
    fn new() -> Self {
        Self {
            mixer_underruns: AtomicU64::new(0),
            mixer_loops: AtomicU64::new(0),
            decoder_stalls: AtomicU64::new(0),
            stream_underruns: AtomicU64::new(0),
        }
    }
}

pub static AUDIO_METRICS: std::sync::LazyLock<AudioMetrics> =
    std::sync::LazyLock::new(AudioMetrics::new);

#[cfg(feature = "telemetry")]
lazy_static! {
    pub static ref METRIC_MIXER_UNDERRUNS: IntCounter = register_int_counter!(
        "audio_mixer_underruns_total",
        "Total number of mixer underruns (starvation)"
    )
    .expect("failed to register audio_mixer_underruns_total");
    pub static ref METRIC_MIXER_BUFFER_DEPTH: IntGauge = register_int_gauge!(
        "audio_mixer_buffer_depth_samples",
        "Current available samples in the mixer buffer"
    )
    .expect("failed to register audio_mixer_buffer_depth_samples");
    pub static ref METRIC_DECODER_STALLS: IntCounter = register_int_counter!(
        "audio_decoder_stalls_total",
        "Total number of decoder stalls (buffer full)"
    )
    .expect("failed to register audio_decoder_stalls_total");
    pub static ref METRIC_STREAM_UNDERRUNS: IntCounter = register_int_counter!(
        "audio_stream_underruns_total",
        "Total number of output stream underruns"
    )
    .expect("failed to register audio_stream_underruns_total");
    pub static ref METRIC_MIXER_ACTIVE_SOURCES: IntGauge = register_int_gauge!(
        "audio_mixer_active_sources",
        "Current number of audio sources being mixed"
    )
    .expect("failed to register audio_mixer_active_sources");
    pub static ref METRIC_MIXER_PROCESSING_DURATION: Histogram = register_histogram!(
        HistogramOpts::new(
            "audio_mixer_processing_duration_seconds",
            "Time taken for a single mixer loop iteration"
        )
        .buckets(vec![
            0.001, 0.005, 0.010, 0.015, 0.020, 0.025, 0.030, 0.050, 0.100
        ])
    )
    .expect("failed to register audio_mixer_processing_duration_seconds");
    pub static ref METRIC_DECODE_ERRORS: CounterVec = register_counter_vec!(
        Opts::new(
            "audio_decode_errors_total",
            "Total number of audio decoding errors"
        ),
        &["error_type"]
    )
    .expect("failed to register audio_decode_errors_total");
    pub static ref METRIC_SESSION_ACTIVE: IntGauge = register_int_gauge!(
        "audio_session_active_total",
        "Number of active audio sessions"
    )
    .expect("failed to register audio_session_active_total");
    pub static ref METRIC_TRACK_LIFECYCLE: CounterVec = register_counter_vec!(
        Opts::new("audio_track_lifecycle_total", "Track lifecycle events"),
        &["event", "queue_name"]
    )
    .expect("failed to register audio_track_lifecycle_total");
    pub static ref METRIC_PRELOAD: CounterVec = register_counter_vec!(
        Opts::new("audio_preload_total", "Audio preload attempts"),
        &["result"]
    )
    .expect("failed to register audio_preload_total");
    pub static ref METRIC_TAPHUB_REQUEST_DURATION: Histogram = register_histogram!(
        HistogramOpts::new(
            "taphub_request_duration_seconds",
            "Latency of TapHub API requests"
        )
        .buckets(vec![
            0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.0, 2.5, 5.0, 10.0
        ])
    )
    .expect("failed to register taphub_request_duration_seconds");
    pub static ref METRIC_TAPHUB_ERRORS: CounterVec = register_counter_vec!(
        Opts::new(
            "taphub_errors_total",
            "Total number of TapHub request errors"
        ),
        &["endpoint"]
    )
    .expect("failed to register taphub_errors_total");
}

pub fn record_mixer_underrun() {
    AUDIO_METRICS
        .mixer_underruns
        .fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "telemetry")]
    METRIC_MIXER_UNDERRUNS.inc();
}

pub fn record_mixer_buffer_depth(depth: usize) {
    #[cfg(feature = "telemetry")]
    METRIC_MIXER_BUFFER_DEPTH.set(depth as i64);
}

pub fn record_decoder_stall() {
    AUDIO_METRICS.decoder_stalls.fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "telemetry")]
    METRIC_DECODER_STALLS.inc();
}

pub fn record_stream_underrun() {
    AUDIO_METRICS
        .stream_underruns
        .fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "telemetry")]
    METRIC_STREAM_UNDERRUNS.inc();
}

pub fn record_mixer_active_sources(count: i64) {
    #[cfg(feature = "telemetry")]
    METRIC_MIXER_ACTIVE_SOURCES.set(count);
}

pub fn inc_mixer_active_sources() {
    #[cfg(feature = "telemetry")]
    METRIC_MIXER_ACTIVE_SOURCES.inc();
}

pub fn dec_mixer_active_sources() {
    #[cfg(feature = "telemetry")]
    METRIC_MIXER_ACTIVE_SOURCES.dec();
}

pub fn record_mixer_processing_duration(duration_secs: f64) {
    #[cfg(feature = "telemetry")]
    METRIC_MIXER_PROCESSING_DURATION.observe(duration_secs);
}

pub fn record_decode_error(error_type: &str) {
    #[cfg(feature = "telemetry")]
    METRIC_DECODE_ERRORS.with_label_values(&[error_type]).inc();
}

pub fn inc_session_active() {
    #[cfg(feature = "telemetry")]
    METRIC_SESSION_ACTIVE.inc();
}

pub fn dec_session_active() {
    #[cfg(feature = "telemetry")]
    METRIC_SESSION_ACTIVE.dec();
}

pub fn record_track_lifecycle(event: &str, queue_name: &str) {
    #[cfg(feature = "telemetry")]
    METRIC_TRACK_LIFECYCLE
        .with_label_values(&[event, queue_name])
        .inc();
}

pub fn record_preload(result: &str) {
    #[cfg(feature = "telemetry")]
    METRIC_PRELOAD.with_label_values(&[result]).inc();
}

pub fn record_taphub_request_duration(duration_secs: f64) {
    #[cfg(feature = "telemetry")]
    METRIC_TAPHUB_REQUEST_DURATION.observe(duration_secs);
}

pub fn record_taphub_error(endpoint: &str) {
    #[cfg(feature = "telemetry")]
    METRIC_TAPHUB_ERRORS.with_label_values(&[endpoint]).inc();
}
