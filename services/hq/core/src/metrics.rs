//! HQ's own OpenTelemetry counters.
//!
//! These are exported through the global meter provider that
//! `zako3_telemetry::init` installs in `hq-boot`, so they show up both on the
//! service's `/metrics` endpoint and over OTLP when it is configured.
//!
//! The instrument set is deliberately small: the play/drop counters are the
//! ones that answer "did a message that a user typed actually reach the audio
//! pipeline, and if not why", which is the question the drop path exists for.

use std::sync::OnceLock;

use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Meter},
};

pub struct HqMetrics {
    /// One increment per attempt sent to the audio engine, labelled with the
    /// attempt number and the result. A message that needed its retry shows up
    /// twice here with `attempt = 1,2`.
    pub play_attempts_total: Counter<u64>,
    /// One increment per message dropped on a play attempt, labelled with the
    /// `dropped_reason`. A message routed to two channels increments once per
    /// channel it failed to reach.
    pub messages_dropped_total: Counter<u64>,
    /// One increment per message that never reached the engine because there
    /// was nothing to play it into: no Zako bot connected anywhere in the
    /// guild. Kept apart from `messages_dropped_total` on purpose. That counter
    /// is the audit's "a play failed" signal and deliberately leaves ordinary
    /// states out of it; this one exists because an ordinary state is still a
    /// message a user typed and nobody heard.
    pub messages_unreadable_total: Counter<u64>,
}

static METRICS: OnceLock<HqMetrics> = OnceLock::new();

impl HqMetrics {
    /// Build the instruments on a specific meter.
    ///
    /// [`metrics`] passes the process-wide global meter; the test below passes
    /// one whose reader is a Prometheus registry, which is how "the drop
    /// counter really reaches `/metrics`" is checked without a cluster.
    pub fn new(meter: &Meter) -> Self {
        Self {
            play_attempts_total: meter
                .u64_counter("hq_play_attempts_total")
                .with_description(
                    "Play attempts sent to the audio engine, by attempt number and result",
                )
                .build(),
            messages_dropped_total: meter
                .u64_counter("hq_messages_dropped_total")
                .with_description(
                    "Messages dropped on a play attempt, by dropped_reason \
                     (one increment per channel a message failed to reach)",
                )
                .build(),
            messages_unreadable_total: meter
                .u64_counter("hq_messages_unreadable_total")
                .with_description(
                    "Messages that never reached the engine because no Zako bot was \
                     connected in the guild, by reason",
                )
                .build(),
        }
    }
}

/// The process-wide HQ instruments.
///
/// The meter provider must have been installed before the first call (it is,
/// in `hq-boot`), otherwise these instruments are no-ops.
pub fn metrics() -> &'static HqMetrics {
    METRICS.get_or_init(|| HqMetrics::new(&global::meter("hq")))
}

/// Record one play attempt (`attempt` is 1-based) and whether it succeeded.
pub fn record_play_attempt(attempt: u32, ok: bool) {
    record_play_attempt_on(metrics(), attempt, ok);
}

fn record_play_attempt_on(metrics: &HqMetrics, attempt: u32, ok: bool) {
    metrics.play_attempts_total.add(
        1,
        &[
            KeyValue::new("attempt", i64::from(attempt)),
            KeyValue::new("result", if ok { "ok" } else { "error" }),
        ],
    );
}

/// Record one message that was dropped, keyed by the same string the log line
/// carries in `dropped_reason`.
pub fn record_message_dropped(reason: &str) {
    record_message_dropped_on(metrics(), reason);
}

fn record_message_dropped_on(metrics: &HqMetrics, reason: &str) {
    metrics
        .messages_dropped_total
        .add(1, &[KeyValue::new("dropped_reason", reason.to_string())]);
}

/// Record one message that could not be played because the guild had no Zako
/// session at all.
///
/// This is the counter that answers "is Zako quietly ignoring a channel",
/// which the drop counter cannot: a message with nowhere to go never reached
/// the engine, so it is not a failed play.
pub fn record_message_unreadable(reason: &str) {
    record_message_unreadable_on(metrics(), reason);
}

fn record_message_unreadable_on(metrics: &HqMetrics, reason: &str) {
    metrics
        .messages_unreadable_total
        .add(1, &[KeyValue::new("reason", reason.to_string())]);
}

#[cfg(test)]
mod tests {
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::SdkMeterProvider;

    use super::*;

    #[test]
    fn recording_without_a_provider_is_harmless() {
        // No meter provider is installed in a unit test; the global meter is a
        // no-op meter, and recording must not panic.
        record_play_attempt(1, false);
        record_message_dropped("tap_unavailable");
        record_message_unreadable("no_bot_session");
    }

    /// The drop counter is only useful if an operator can see it. This wires
    /// the instruments to a Prometheus registry the way
    /// `crates/telemetry/src/metrics.rs` wires the service's provider, and
    /// asserts the scrape carries both counters under the names the audit, the
    /// logs and the runbooks use.
    #[test]
    fn both_counters_reach_a_prometheus_scrape_under_their_documented_names() {
        let registry = prometheus::Registry::new();
        let exporter = opentelemetry_prometheus::exporter()
            .with_registry(registry.clone())
            .without_counter_suffixes()
            .build()
            .expect("prometheus exporter builds");
        let provider = SdkMeterProvider::builder().with_reader(exporter).build();

        let metrics = HqMetrics::new(&provider.meter("hq"));
        record_play_attempt_on(&metrics, 1, false);
        record_play_attempt_on(&metrics, 2, true);
        record_message_dropped_on(&metrics, "tap_unavailable");
        record_message_unreadable_on(&metrics, "no_bot_session");

        let families = registry.gather();
        let names: Vec<String> = families.iter().map(|f| f.name().to_string()).collect();

        let attempts = family(&families, "hq_play_attempts_total", &names);
        let retried = point(attempts, &[("attempt", "1"), ("result", "error")]);
        assert_eq!(retried, 1.0, "the failed first attempt is counted");
        let succeeded = point(attempts, &[("attempt", "2"), ("result", "ok")]);
        assert_eq!(succeeded, 1.0, "the retry that worked is counted");

        let dropped = family(&families, "hq_messages_dropped_total", &names);
        assert_eq!(
            point(dropped, &[("dropped_reason", "tap_unavailable")]),
            1.0,
            "a drop is counted once under its reason"
        );

        let unreadable = family(&families, "hq_messages_unreadable_total", &names);
        assert_eq!(
            point(unreadable, &[("reason", "no_bot_session")]),
            1.0,
            "a message with nowhere to play is counted where an operator looks"
        );
    }

    fn family<'a>(
        families: &'a [prometheus::proto::MetricFamily],
        name: &str,
        names: &[String],
    ) -> &'a prometheus::proto::MetricFamily {
        families
            .iter()
            .find(|f| f.name() == name)
            .unwrap_or_else(|| panic!("`{name}` is not in the scrape; families: {names:?}"))
    }

    /// The value of the first sample carrying these labels (a subset test: the
    /// exporter adds `otel_scope_name` of its own).
    fn point(family: &prometheus::proto::MetricFamily, labels: &[(&str, &str)]) -> f64 {
        family
            .get_metric()
            .iter()
            .find(|m| {
                labels.iter().all(|(name, value)| {
                    m.get_label()
                        .iter()
                        .any(|l| l.name() == *name && l.value() == *value)
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "no sample of `{}` with {labels:?}; samples: {:?}",
                    family.name(),
                    family
                        .get_metric()
                        .iter()
                        .map(|m| m
                            .get_label()
                            .iter()
                            .map(|l| format!("{}={}", l.name(), l.value()))
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                )
            })
            .get_counter()
            .value()
    }
}
