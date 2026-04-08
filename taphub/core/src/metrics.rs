use opentelemetry::{
    KeyValue,
    global,
    metrics::{Counter, Histogram, UpDownCounter},
};
use std::sync::OnceLock;

pub struct TapHubMetrics {
    pub connected_taps: UpDownCounter<i64>,
    pub tap_auth_total: Counter<u64>,
    pub audio_requests_total: Counter<u64>,
    pub audio_request_duration: Histogram<f64>,
    pub cache_hits_total: Counter<u64>,
    pub connection_duration: Histogram<f64>,
    pub active_streams: UpDownCounter<i64>,
}

static METRICS: OnceLock<TapHubMetrics> = OnceLock::new();

/// Returns the process-wide TapHub OTel metrics.
///
/// The global meter provider must have been set (via `zako3_telemetry::init`) before the
/// first call, otherwise instruments will be no-ops.
pub fn metrics() -> &'static TapHubMetrics {
    METRICS.get_or_init(|| {
        let meter = global::meter("taphub");
        TapHubMetrics {
            connected_taps: meter
                .i64_up_down_counter("taphub_connected_taps")
                .with_description("Number of currently connected taps")
                .build(),
            tap_auth_total: meter
                .u64_counter("taphub_tap_auth_total")
                .with_description("Total tap authentication attempts")
                .build(),
            audio_requests_total: meter
                .u64_counter("taphub_audio_requests_total")
                .with_description("Total audio requests handled")
                .build(),
            audio_request_duration: meter
                .f64_histogram("taphub_audio_request_duration_seconds")
                .with_description("Audio request duration in seconds")
                .with_unit("s")
                .build(),
            cache_hits_total: meter
                .u64_counter("taphub_cache_hits_total")
                .with_description("Total audio cache hits")
                .build(),
            connection_duration: meter
                .f64_histogram("taphub_connection_duration_seconds")
                .with_description("Tap connection duration in seconds")
                .with_unit("s")
                .build(),
            active_streams: meter
                .i64_up_down_counter("taphub_active_streams")
                .with_description("Number of active audio relay streams")
                .build(),
        }
    })
}

/// Convenience: record an audio request and its outcome.
pub fn record_audio_request(tap_id: &str, cache_hit: bool, duration_secs: f64, ok: bool) {
    let m = metrics();
    let result = if ok {
        if cache_hit { "cache_hit" } else { "ok" }
    } else {
        "error"
    };
    m.audio_requests_total.add(
        1,
        &[
            KeyValue::new("tap_id", tap_id.to_string()),
            KeyValue::new("result", result),
        ],
    );
    m.audio_request_duration.record(
        duration_secs,
        &[
            KeyValue::new("tap_id", tap_id.to_string()),
            KeyValue::new("cache_hit", cache_hit.to_string()),
        ],
    );
}
