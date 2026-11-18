use opentelemetry::{
    KeyValue, global,
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
    /// Tap connections that announced a drain, by reason.
    ///
    /// A connection that leaves this way is a planned departure; one that does
    /// not is a lost connection. They look identical in `connection_duration`,
    /// which is why leaving is counted on its own.
    pub tap_conn_draining_total: Counter<u64>,
    /// Tap-stated drain start → out of the routing table, in seconds.
    ///
    /// Measured from the timestamp the tap put in its notice, so it covers the
    /// whole hop rather than just this process's handling of it.
    pub tap_conn_drain_to_removal_seconds: Histogram<f64>,
    /// Requests that found no live connection for their tap.
    ///
    /// `TapUnavailable` covers "the tap was never here" and "every replica is
    /// draining" with one wire reason; this is where the two are told apart.
    pub no_tap_available_total: Counter<u64>,
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
            tap_conn_draining_total: meter
                .u64_counter("taphub_tap_conn_draining_total")
                .with_description("Tap connections that announced a drain before leaving")
                .build(),
            tap_conn_drain_to_removal_seconds: meter
                .f64_histogram("taphub_tap_conn_drain_to_removal_seconds")
                .with_description(
                    "Time from a tap starting its drain to its connection leaving the routing table",
                )
                .with_unit("s")
                .build(),
            no_tap_available_total: meter
                .u64_counter("taphub_no_tap_available_total")
                .with_description("Requests for a tap that had no routable connection")
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
