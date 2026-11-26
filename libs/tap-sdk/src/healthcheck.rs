//! The tap's HTTP surface: `/health` and `/metrics`.
//!
//! `/health` used to answer a constant `"ok"`, which is why a tap that had
//! stopped serving requests stayed `1/1 Running` for hours: the readiness probe
//! asked a question whose answer never changed. It now reads the request-path
//! state ([`zakofish::TapMetrics`]) and reports unhealthy when a request is in
//! flight and nothing has moved for [`DEFAULT_STALL_THRESHOLD`].
//!
//! Idle is healthy. A tap with no requests is not stuck, it is unused, and
//! failing it would restart every replica whenever the voice channel is quiet.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};

use zakofish::{TapMetrics, TapSnapshot, metrics::unix_millis};

/// How long a request may sit without a reply before `/health` calls the tap
/// stuck.
///
/// Above the slowest request any tap serves (supertonic synthesis: 753ms worst
/// case measured over a day), so a normally slow synthesis cannot fail its own
/// readiness probe, and far below the many minutes a wedged tap used to sit
/// there before somebody noticed. Overridable with `TAP_HEALTH_STALL_SECS`.
pub const DEFAULT_STALL_THRESHOLD: Duration = Duration::from_secs(20);

/// What the health endpoint reads.
#[derive(Clone)]
pub(crate) struct HealthState {
    pub metrics: Arc<TapMetrics>,
    pub stall_threshold: Duration,
}

pub(crate) async fn run_healthcheck_server(port: u16, state: HealthState) {
    match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => {
            tracing::info!(
                port,
                stall_threshold_ms = state.stall_threshold.as_millis() as u64,
                "healthcheck and metrics listening"
            );
            if let Err(e) = axum::serve(listener, router(state)).await {
                tracing::error!("Healthcheck server error: {e}");
            }
        }
        Err(e) => tracing::error!("Failed to bind healthcheck on port {port}: {e}"),
    }
}

/// The routes, with the wall clock read as requests arrive.
pub(crate) fn router(state: HealthState) -> Router {
    router_with_clock(state, unix_millis)
}

/// [`router`] with the clock supplied by the caller.
///
/// Exists so a test can put "now" past the threshold without sleeping or
/// reaching into the counters: the handler under test is the real one, only the
/// reading of the clock is moved.
pub(crate) fn router_with_clock(state: HealthState, clock: fn() -> u64) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .with_state((state, clock))
}

async fn health_handler(
    State((state, clock)): State<(HealthState, fn() -> u64)>,
) -> impl IntoResponse {
    let snapshot = state.metrics.snapshot();
    let now_ms = clock();
    let silent_ms = now_ms.saturating_sub(snapshot.last_activity_at_ms);
    let status = verdict(&snapshot, now_ms, state.stall_threshold);

    let body = if status == StatusCode::OK {
        format!(
            "ok in_flight={} silent_ms={silent_ms}",
            snapshot.in_flight
        )
    } else {
        format!(
            "stalled in_flight={} silent_ms={silent_ms} threshold_ms={}",
            snapshot.in_flight,
            state.stall_threshold.as_millis() as u64
        )
    };

    if status != StatusCode::OK {
        tracing::warn!(
            in_flight = snapshot.in_flight,
            silent_ms,
            last_request_at_ms = snapshot.last_request_at_ms,
            last_reply_at_ms = snapshot.last_reply_at_ms,
            "health check: a request is in flight and nothing has moved past the threshold"
        );
    }

    (status, body)
}

/// The decision, as a pure function of a snapshot and the clock.
///
/// Unhealthy means *a request is being served and nothing has come of it*. Both
/// halves matter:
///
/// - In flight with no reply past the threshold is the S2b signature — the
///   request was taken off the wire and then nothing.
/// - Idle is healthy even if the last reply was hours ago: a quiet channel is
///   not a stuck tap, and a probe that says otherwise restarts the voice for
///   being unused.
///
/// Silence is measured from the last *activity* (a request received or a reply
/// written), not from the last reply alone. A tap that has been idle for an
/// hour and then receives a request has no reply yet; measuring from the reply
/// would fail that request the moment it arrived.
pub(crate) fn verdict(
    snapshot: &TapSnapshot,
    now_ms: u64,
    stall_threshold: Duration,
) -> StatusCode {
    if snapshot.in_flight == 0 {
        return StatusCode::OK;
    }

    let silent_ms = now_ms.saturating_sub(snapshot.last_activity_at_ms);
    if silent_ms > stall_threshold.as_millis() as u64 {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

async fn metrics_handler() -> impl IntoResponse {
    #[cfg(feature = "metrics")]
    {
        use prometheus::Encoder;

        let encoder = prometheus::TextEncoder::new();
        let mut buffer = Vec::new();
        match encoder.encode(&prometheus::gather(), &mut buffer) {
            Ok(()) => (StatusCode::OK, String::from_utf8_lossy(&buffer).into_owned()),
            Err(e) => {
                tracing::error!("failed to encode metrics: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    String::from("failed to encode metrics\n"),
                )
            }
        }
    }

    #[cfg(not(feature = "metrics"))]
    {
        (
            StatusCode::NOT_FOUND,
            String::from("this tap was built without the `metrics` feature\n"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zakofish::RequestKind;

    fn state(metrics: Arc<TapMetrics>, stall_threshold: Duration) -> HealthState {
        HealthState {
            metrics,
            stall_threshold,
        }
    }

    /// GET `/health` over a real socket, so the test covers the handler and the
    /// status code a probe would see, not only the decision function.
    async fn get_health(state: HealthState, clock: fn() -> u64) -> (u16, String) {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind an ephemeral port");
        let port = listener.local_addr().expect("local addr").port();

        let app = router_with_clock(state, clock);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect to the healthcheck server");
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("write the request");

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read the response");
        let response = String::from_utf8_lossy(&response).into_owned();

        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or_else(|| panic!("no status line in: {response}"));

        (status, response)
    }

    /// A tap nobody is using answers 200, however long it has been quiet.
    #[tokio::test]
    async fn an_idle_tap_is_healthy() {
        let metrics = TapMetrics::new();
        let (status, body) = get_health(
            state(metrics, DEFAULT_STALL_THRESHOLD),
            unix_millis,
        )
        .await;

        assert_eq!(status, 200, "{body}");
        assert!(body.contains("in_flight=0"), "{body}");
    }

    /// The S2b shape: a request was taken off the wire and then nothing came of
    /// it. `/health` must say 503, which is what a readiness probe (and a
    /// liveness probe, once one exists) acts on.
    #[tokio::test]
    async fn a_request_in_flight_with_no_reply_past_the_threshold_is_unhealthy() {
        let metrics = TapMetrics::new();
        let guard = metrics.request_received(RequestKind::Audio);

        // Thirty seconds after the request arrived, still nothing written.
        let stalled_clock = || unix_millis() + 30_000;
        let (status, body) = get_health(
            state(Arc::clone(&metrics), DEFAULT_STALL_THRESHOLD),
            stalled_clock,
        )
        .await;

        assert_eq!(status, 503, "{body}");
        assert!(body.contains("stalled"), "{body}");
        assert!(body.contains("in_flight=1"), "{body}");

        drop(guard);
    }

    /// A request that just arrived, or one that is streaming, is healthy: the
    /// probe must not fail a tap that is doing its job.
    #[tokio::test]
    async fn a_request_making_progress_is_healthy() {
        let metrics = TapMetrics::new();
        let guard = metrics.request_received(RequestKind::Meta);
        metrics.reply_sent();

        let (status, body) = get_health(
            state(Arc::clone(&metrics), DEFAULT_STALL_THRESHOLD),
            unix_millis,
        )
        .await;

        assert_eq!(status, 200, "{body}");

        drop(guard);
    }

    #[test]
    fn the_verdict_ignores_idle_time_and_respects_the_threshold() {
        let idle = TapSnapshot {
            in_flight: 0,
            last_request_at_ms: 1_000,
            last_reply_at_ms: 1_000,
            last_activity_at_ms: 1_000,
        };
        assert_eq!(
            verdict(&idle, 10_000_000, DEFAULT_STALL_THRESHOLD),
            StatusCode::OK,
            "an idle tap is healthy however long it has been idle"
        );

        let serving = TapSnapshot {
            in_flight: 1,
            last_request_at_ms: 99_000,
            last_reply_at_ms: 99_000,
            last_activity_at_ms: 99_000,
        };
        assert_eq!(
            verdict(&serving, 99_000 + 19_999, DEFAULT_STALL_THRESHOLD),
            StatusCode::OK
        );
        assert_eq!(
            verdict(&serving, 99_000 + 20_001, DEFAULT_STALL_THRESHOLD),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    /// A tap that has been idle for an hour and then takes a request must not
    /// fail that request instantly: the silence is measured from what just
    /// arrived, not from the reply that happened before it.
    #[test]
    fn a_request_that_just_arrived_after_a_long_idle_is_not_a_stall() {
        let now = 3_600_000u64;
        let snapshot = TapSnapshot {
            in_flight: 1,
            last_request_at_ms: now,
            last_reply_at_ms: now - 3_600_000,
            last_activity_at_ms: now,
        };
        assert_eq!(
            verdict(&snapshot, now, DEFAULT_STALL_THRESHOLD),
            StatusCode::OK
        );
    }

    /// The series the runbooks and the milestone name, exactly as they will
    /// appear on `/metrics` — the exporter does not add `_total` or a unit
    /// suffix to a name that already carries one.
    #[cfg(feature = "metrics")]
    #[test]
    fn the_scrape_carries_the_documented_series() {
        zako3_telemetry::init_metrics("zako3-tap-test", None).expect("init metrics");

        let metrics = TapMetrics::instrumented();
        let meta = metrics.request_received(RequestKind::Meta);
        metrics.reply_sent();
        metrics.chan_opened();
        metrics.chan_closed();
        metrics.accept_chan_error();
        metrics.handshake();
        // Finished before the scrape, so `tap_requests_completed_total` has
        // something to show; the audio request stays in flight so the gauge is
        // a level and not a total.
        drop(meta);
        let audio = metrics.request_received(RequestKind::Audio);

        let families = prometheus::gather();
        let names: Vec<String> = families.iter().map(|f| f.name().to_string()).collect();

        for expected in [
            "tap_requests_received_total",
            "tap_requests_completed_total",
            "tap_in_flight",
            "tap_last_request_at_ms",
            "tap_last_reply_at_ms",
            "tap_chan_open_total",
            "tap_chan_closed_total",
            "tap_accept_chan_errors_total",
            "tap_handshake_total",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "`{expected}` is missing from the scrape; families: {names:?}"
            );
        }

        let received = families
            .iter()
            .find(|f| f.name() == "tap_requests_received_total")
            .expect("requests received");
        let meta = received
            .get_metric()
            .iter()
            .find(|m| {
                m.get_label()
                    .iter()
                    .any(|l| l.name() == "kind" && l.value() == "meta")
            })
            .expect("a `kind=\"meta\"` sample");
        assert_eq!(meta.get_counter().value(), 1.0);

        let completed = families
            .iter()
            .find(|f| f.name() == "tap_requests_completed_total")
            .expect("requests completed");
        assert_eq!(completed.get_metric()[0].get_counter().value(), 1.0);

        // The gauge has to be the value right now, not a total since start.
        let in_flight = families
            .iter()
            .find(|f| f.name() == "tap_in_flight")
            .expect("in flight");
        assert_eq!(in_flight.get_metric()[0].get_gauge().value(), 1.0);

        drop(audio);
    }

    #[test]
    fn the_stall_threshold_default_is_twenty_seconds() {
        assert_eq!(DEFAULT_STALL_THRESHOLD, Duration::from_secs(20));
    }
}
