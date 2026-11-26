use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge, Histogram, UpDownCounter},
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
    /// Tap connections taken out of the routing set because they stopped
    /// answering (`TapHub::mark_unanswered`).
    ///
    /// Separate from `tap_conn_draining_total`: a drain is the tap saying it is
    /// leaving, this is taphub finding out that a connection which still claims
    /// to be here does not answer.
    pub tap_conn_unresponsive_total: Counter<u64>,
    /// Seconds each live connection has gone without answering anything,
    /// labelled by `connection_id` (and `tap_id`).
    ///
    /// The per-connection form of the `silent_secs` the eviction path already
    /// computes. A connection frozen at a growing silence while its neighbours
    /// answer is S2b, and before this gauge that number existed only inside one
    /// log line per eviction.
    pub tap_conn_silent_seconds: Gauge<f64>,
    /// Times each live connection has been retired without answering anything
    /// in between (`ConnEntry.suspect_strikes`), labelled by `connection_id`.
    ///
    /// Zero means the connection is in normal rotation; above zero it is on
    /// probation and gets half the metadata budget (`handler/meta.rs`).
    pub tap_conn_strikes: Gauge<u64>,
    /// Request chans opened on a connection that have not been answered yet,
    /// labelled by `connection_id`.
    ///
    /// protofish3 has no chan-close, so a request abandoned on timeout leaves
    /// its chan open on the wire: `streams_opened − streams_closed` is the
    /// direct, cumulative measurement of that leak, and the only direct
    /// evidence for or against a stream-credit exhaustion.
    pub tap_conn_inflight_requests: Gauge<i64>,
    /// Request chans opened on a connection, cumulative, labelled by
    /// `connection_id`.
    ///
    /// One per call to `ZakofishHub::request_audio`/`request_audio_metadata`,
    /// which each open exactly one chan (`conn.open_chan()`).
    pub tap_conn_streams_opened_total: Counter<u64>,
    /// Request chans on a connection that got an answer, cumulative, labelled
    /// by `connection_id`.
    ///
    /// A tap refusal counts: the chan completed, whatever the answer said. A
    /// timeout, a transport error or a disconnect does not, because none of
    /// them closes the chan on a protofish3 connection.
    pub tap_conn_streams_closed_total: Counter<u64>,
    /// Tap connections that went away, by `reason`
    /// (`drain`/`clean`/`close_code`/`no_close`).
    ///
    /// `no_close` is the one worth watching: the peer never said goodbye, which
    /// is what a process that died or stopped reading looks like from here.
    pub tap_conn_closed_total: Counter<u64>,
    /// Metadata requests sent to a tap, by `attempt` (1 is the first
    /// connection tried).
    ///
    /// `attempt=2` is the retry `handler/meta.rs` makes when a connection on
    /// probation does not answer inside its half budget.
    pub meta_attempts_total: Counter<u64>,
    /// Metadata requests that were handed to a second connection.
    pub meta_retry_total: Counter<u64>,
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
            tap_conn_unresponsive_total: meter
                .u64_counter("taphub_tap_conn_unresponsive_total")
                .with_description("Tap connections taken out of routing after failing to answer")
                .build(),
            tap_conn_silent_seconds: meter
                .f64_gauge("taphub_tap_conn_silent_seconds")
                .with_description("Seconds this tap connection has gone without answering anything")
                .with_unit("s")
                .build(),
            tap_conn_strikes: meter
                .u64_gauge("taphub_tap_conn_strikes")
                .with_description(
                    "Times this tap connection was retired without answering anything in between",
                )
                .build(),
            tap_conn_inflight_requests: meter
                .i64_gauge("taphub_tap_conn_inflight_requests")
                .with_description("Request chans opened on this tap connection and not answered")
                .build(),
            tap_conn_streams_opened_total: meter
                .u64_counter("taphub_tap_conn_streams_opened_total")
                .with_description("Request chans opened on tap connections")
                .build(),
            tap_conn_streams_closed_total: meter
                .u64_counter("taphub_tap_conn_streams_closed_total")
                .with_description("Request chans on tap connections that got an answer")
                .build(),
            tap_conn_closed_total: meter
                .u64_counter("taphub_tap_conn_closed_total")
                .with_description("Tap connections that went away, by reason")
                .build(),
            meta_attempts_total: meter
                .u64_counter("taphub_meta_attempts_total")
                .with_description("Metadata requests sent to a tap, by attempt number")
                .build(),
            meta_retry_total: meter
                .u64_counter("taphub_meta_retry_total")
                .with_description("Metadata requests retried on another tap connection")
                .build(),
        }
    })
}

/// The label set every per-connection series carries: the connection it is
/// about, and the tap it belongs to (so a series can be read without a join).
pub fn connection_key_values(connection_id: u64, tap_id: &str) -> [KeyValue; 2] {
    [
        KeyValue::new("connection_id", connection_id.to_string()),
        KeyValue::new("tap_id", tap_id.to_string()),
    ]
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

/// A request chan was opened on this connection: the tap is about to be asked.
pub fn record_stream_opened(connection_id: u64, tap_id: &str) {
    metrics()
        .tap_conn_streams_opened_total
        .add(1, &connection_key_values(connection_id, tap_id));
}

/// A request chan on this connection got an answer, of any kind.
pub fn record_stream_closed(connection_id: u64, tap_id: &str) {
    metrics()
        .tap_conn_streams_closed_total
        .add(1, &connection_key_values(connection_id, tap_id));
}

/// One more metadata attempt, on the connection at position `attempt` of this
/// request (1-based).
pub fn record_meta_attempt(attempt: u64) {
    metrics()
        .meta_attempts_total
        .add(1, &[KeyValue::new("attempt", attempt.to_string())]);
}

/// A metadata request was handed to another connection.
pub fn record_meta_retry() {
    metrics().meta_retry_total.add(1, &[]);
}
