//! Request-path instrumentation for a pf3 tap.
//!
//! Two readers share this state, which is why it lives beside the transport
//! rather than in the tap binary:
//!
//! - a `/metrics` scrape, which wants counters and timestamps, and
//! - `/health`, which wants the one thing a scrape can only show after the fact:
//!   *is a request being served right now, and has anything moved since it
//!   arrived?*
//!
//! The atomics below are maintained whether or not the `metrics` feature is on,
//! so a tap that never installs a meter provider still answers `/health`
//! truthfully. The OpenTelemetry instruments are built only with the feature,
//! and record into whatever meter provider the caller installed — for the taps
//! that is `zako3-telemetry::init_metrics`, so the names on `/metrics` are the
//! instrument names, unchanged (see that crate's `prometheus_exporter`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "metrics")]
use opentelemetry::KeyValue;
#[cfg(feature = "metrics")]
use opentelemetry::metrics::{Counter, Gauge};

/// The `kind` label a metadata request carries.
pub const REQUEST_KIND_META: &str = "meta";
/// The `kind` label an audio request carries.
pub const REQUEST_KIND_AUDIO: &str = "audio";

/// Which half of the request path a request arrived on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    /// `AudioMetadataRequest` — the cheap lookup every message makes first.
    Meta,
    /// `AudioRequest` — the one that streams.
    Audio,
}

impl RequestKind {
    /// The value this kind carries in the `kind` label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Meta => REQUEST_KIND_META,
            Self::Audio => REQUEST_KIND_AUDIO,
        }
    }
}

/// Wall-clock milliseconds since the unix epoch.
pub fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Everything `/health` needs, read in one go so the three values agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapSnapshot {
    /// Requests being served right now. The same number a drain waits on.
    pub in_flight: u32,
    /// When a request was last taken off the wire.
    pub last_request_at_ms: u64,
    /// When a response (or an audio frame) was last written back to the hub.
    pub last_reply_at_ms: u64,
    /// The later of the two — the last moment the tap demonstrably did
    /// something about a request.
    pub last_activity_at_ms: u64,
}

/// Request-path state for one tap connection.
pub struct TapMetrics {
    in_flight: AtomicU32,
    last_request_at_ms: AtomicU64,
    last_reply_at_ms: AtomicU64,
    last_activity_at_ms: AtomicU64,
    #[cfg(feature = "metrics")]
    instruments: Option<Instruments>,
}

#[cfg(feature = "metrics")]
struct Instruments {
    requests_received: Counter<u64>,
    requests_completed: Counter<u64>,
    in_flight: Gauge<u64>,
    last_request_at_ms: Gauge<u64>,
    last_reply_at_ms: Gauge<u64>,
    chan_open: Counter<u64>,
    chan_closed: Counter<u64>,
    accept_chan_errors: Counter<u64>,
    handshake: Counter<u64>,
}

impl TapMetrics {
    /// Counters with no exporter behind them.
    ///
    /// Used by taps that do not build the `metrics` feature, and by tests that
    /// only care about the `/health` inputs.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            in_flight: AtomicU32::new(0),
            last_request_at_ms: AtomicU64::new(0),
            last_reply_at_ms: AtomicU64::new(0),
            last_activity_at_ms: AtomicU64::new(0),
            #[cfg(feature = "metrics")]
            instruments: None,
        })
    }

    /// Counters that also record into the global meter provider.
    ///
    /// The instruments bind to the provider that is installed *when this is
    /// called*: call `zako3_telemetry::init_metrics` first, or the recordings go
    /// to the no-op provider and `/metrics` stays empty.
    #[cfg(feature = "metrics")]
    pub fn instrumented() -> Arc<Self> {
        let meter = opentelemetry::global::meter("zako3-tap");
        let instruments = Instruments {
            requests_received: meter
                .u64_counter("tap_requests_received_total")
                .with_description("Requests taken off the wire, by kind")
                .build(),
            requests_completed: meter
                .u64_counter("tap_requests_completed_total")
                .with_description("Requests that finished, however they ended")
                .build(),
            in_flight: meter
                .u64_gauge("tap_in_flight")
                .with_description("Requests being served right now")
                .build(),
            last_request_at_ms: meter
                .u64_gauge("tap_last_request_at_ms")
                .with_description("Unix ms when a request was last received")
                .build(),
            last_reply_at_ms: meter
                .u64_gauge("tap_last_reply_at_ms")
                .with_description("Unix ms when a response or audio frame was last written")
                .build(),
            chan_open: meter
                .u64_counter("tap_chan_open_total")
                .with_description("Channels accepted from the hub")
                .build(),
            chan_closed: meter
                .u64_counter("tap_chan_closed_total")
                .with_description("Channels whose handler has finished")
                .build(),
            accept_chan_errors: meter
                .u64_counter("tap_accept_chan_errors_total")
                .with_description("accept_chan failures — the connection is being dropped")
                .build(),
            handshake: meter
                .u64_counter("tap_handshake_total")
                .with_description("Handshakes and re-handshakes that the hub accepted")
                .build(),
        };
        // Publish one sample per series before any traffic.
        //
        // An OpenTelemetry instrument with nothing recorded is absent from the
        // scrape, not zero, so a fresh tap would answer `/metrics` with only
        // whatever had happened since it started — and a dashboard asking "how
        // many requests arrived" would get "no data" on exactly the replica
        // that has served nothing yet, which is the state worth looking at.
        // Recording zeros materialises every documented series at startup.
        instruments.requests_received.add(0, &[KeyValue::new("kind", REQUEST_KIND_META)]);
        instruments.requests_received.add(0, &[KeyValue::new("kind", REQUEST_KIND_AUDIO)]);
        instruments.requests_completed.add(0, &[]);
        instruments.in_flight.record(0, &[]);
        instruments.last_request_at_ms.record(0, &[]);
        instruments.last_reply_at_ms.record(0, &[]);
        instruments.chan_open.add(0, &[]);
        instruments.chan_closed.add(0, &[]);
        instruments.accept_chan_errors.add(0, &[]);
        instruments.handshake.add(0, &[]);

        Arc::new(Self {
            in_flight: AtomicU32::new(0),
            last_request_at_ms: AtomicU64::new(0),
            last_reply_at_ms: AtomicU64::new(0),
            last_activity_at_ms: AtomicU64::new(0),
            #[cfg(feature = "metrics")]
            instruments: Some(instruments),
        })
    }

    /// Requests being served right now.
    pub fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// The values `/health` decides on.
    pub fn snapshot(&self) -> TapSnapshot {
        TapSnapshot {
            in_flight: self.in_flight.load(Ordering::SeqCst),
            last_request_at_ms: self.last_request_at_ms.load(Ordering::SeqCst),
            last_reply_at_ms: self.last_reply_at_ms.load(Ordering::SeqCst),
            last_activity_at_ms: self.last_activity_at_ms.load(Ordering::SeqCst),
        }
    }

    /// Counts a request in and hands back the guard that counts it out.
    ///
    /// The guard is dropped however the request ends — answered, refused,
    /// failed, or panicked with `catch_unwind` nowhere in sight — because a
    /// drop is the only exit every path shares.
    pub fn request_received(self: &Arc<Self>, kind: RequestKind) -> RequestGuard {
        let now = unix_millis();
        let in_flight = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.last_request_at_ms.store(now, Ordering::SeqCst);
        self.last_activity_at_ms.store(now, Ordering::SeqCst);

        #[cfg(feature = "metrics")]
        if let Some(i) = &self.instruments {
            i.requests_received
                .add(1, &[KeyValue::new("kind", kind.as_str())]);
            i.in_flight.record(u64::from(in_flight), &[]);
            i.last_request_at_ms.record(now, &[]);
        }

        RequestGuard {
            metrics: Arc::clone(self),
        }
    }

    /// Notes that a response — or one audio frame — went out.
    ///
    /// Called per frame as well as per response: a request that has been
    /// streaming for a minute is making progress, and `/health` must not call
    /// that a stall.
    pub fn reply_sent(&self) {
        let now = unix_millis();
        self.last_reply_at_ms.store(now, Ordering::SeqCst);
        self.last_activity_at_ms.store(now, Ordering::SeqCst);

        #[cfg(feature = "metrics")]
        if let Some(i) = &self.instruments {
            i.last_reply_at_ms.record(now, &[]);
        }
    }

    /// A channel was accepted from the hub.
    pub fn chan_opened(&self) {
        #[cfg(feature = "metrics")]
        if let Some(i) = &self.instruments {
            i.chan_open.add(1, &[]);
        }
    }

    /// A channel's handler finished.
    pub fn chan_closed(&self) {
        #[cfg(feature = "metrics")]
        if let Some(i) = &self.instruments {
            i.chan_closed.add(1, &[]);
        }
    }

    /// `accept_chan` failed.
    pub fn accept_chan_error(&self) {
        #[cfg(feature = "metrics")]
        if let Some(i) = &self.instruments {
            i.accept_chan_errors.add(1, &[]);
        }
    }

    /// A handshake the hub accepted, including a re-handshake after a reconnect.
    pub fn handshake(&self) {
        #[cfg(feature = "metrics")]
        if let Some(i) = &self.instruments {
            i.handshake.add(1, &[]);
        }
    }
}

/// Holds one request in the in-flight count for as long as it is alive.
pub struct RequestGuard {
    metrics: Arc<TapMetrics>,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let left = self
            .metrics
            .in_flight
            .fetch_sub(1, Ordering::SeqCst)
            .saturating_sub(1);

        #[cfg(feature = "metrics")]
        if let Some(i) = &self.metrics.instruments {
            i.requests_completed.add(1, &[]);
            i.in_flight.record(u64::from(left), &[]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_tap_has_nothing_in_flight_and_no_timestamps() {
        let metrics = TapMetrics::new();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.in_flight, 0);
        assert_eq!(snapshot.last_request_at_ms, 0);
        assert_eq!(snapshot.last_reply_at_ms, 0);
    }

    /// The count has to come back down however the request ends, or the drain
    /// waits for a request that is long gone and `/health` reports a stall that
    /// is not happening.
    #[test]
    fn the_guard_puts_the_in_flight_count_back_where_it_found_it() {
        let metrics = TapMetrics::new();

        let first = metrics.request_received(RequestKind::Meta);
        assert_eq!(metrics.in_flight(), 1);
        let second = metrics.request_received(RequestKind::Audio);
        assert_eq!(metrics.in_flight(), 2);

        drop(first);
        assert_eq!(metrics.in_flight(), 1);
        drop(second);
        assert_eq!(metrics.in_flight(), 0);
    }

    #[test]
    fn a_received_request_stamps_the_request_and_activity_time() {
        let metrics = TapMetrics::new();
        let before = unix_millis();
        let guard = metrics.request_received(RequestKind::Meta);

        let snapshot = metrics.snapshot();
        assert!(snapshot.last_request_at_ms >= before);
        assert_eq!(snapshot.last_activity_at_ms, snapshot.last_request_at_ms);
        assert_eq!(snapshot.last_reply_at_ms, 0);

        metrics.reply_sent();
        let snapshot = metrics.snapshot();
        assert!(snapshot.last_reply_at_ms >= snapshot.last_request_at_ms);
        assert_eq!(snapshot.last_activity_at_ms, snapshot.last_reply_at_ms);

        drop(guard);
    }

    #[test]
    fn request_kinds_carry_the_label_values_the_dashboards_use() {
        assert_eq!(RequestKind::Meta.as_str(), "meta");
        assert_eq!(RequestKind::Audio.as_str(), "audio");
    }
}
