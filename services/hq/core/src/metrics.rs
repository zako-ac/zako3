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

use opentelemetry::{KeyValue, global, metrics::Counter};

pub struct HqMetrics {
    /// One increment per attempt sent to the audio engine, labelled with the
    /// attempt number and the result. A message that needed its retry shows up
    /// twice here with `attempt = 1,2`.
    pub play_attempts_total: Counter<u64>,
    /// One increment per message dropped on a play attempt, labelled with the
    /// `dropped_reason`. A message routed to two channels increments once per
    /// channel it failed to reach.
    pub messages_dropped_total: Counter<u64>,
}

static METRICS: OnceLock<HqMetrics> = OnceLock::new();

/// The process-wide HQ instruments.
///
/// The meter provider must have been installed before the first call (it is,
/// in `hq-boot`), otherwise these instruments are no-ops.
pub fn metrics() -> &'static HqMetrics {
    METRICS.get_or_init(|| {
        let meter = global::meter("hq");
        HqMetrics {
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
        }
    })
}

/// Record one play attempt (`attempt` is 1-based) and whether it succeeded.
pub fn record_play_attempt(attempt: u32, ok: bool) {
    metrics().play_attempts_total.add(
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
    metrics()
        .messages_dropped_total
        .add(1, &[KeyValue::new("dropped_reason", reason.to_string())]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_without_a_provider_is_harmless() {
        // No meter provider is installed in a unit test; the global meter is a
        // no-op meter, and recording must not panic.
        record_play_attempt(1, false);
        record_message_dropped("tap_unavailable");
    }
}
