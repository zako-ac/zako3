//! The two metrics the stall device raises.
//!
//! Cardinality is the design constraint: attributes are the bounded enums only
//! (`verdict`, `stage`). The stream, track and per-hop detail live in the log
//! line, where a per-event value costs nothing that outlives the window it is
//! kept for.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use opentelemetry::{KeyValue, global, metrics::Counter};

use crate::{
    inject::Stage,
    report::{HopView, Verdict},
};

static STALLS: OnceLock<Counter<u64>> = OnceLock::new();
static INJECTED: OnceLock<Counter<u64>> = OnceLock::new();

fn meter() -> opentelemetry::metrics::Meter {
    global::meter("audio-probe")
}

fn stalls() -> &'static Counter<u64> {
    STALLS.get_or_init(|| {
        meter()
            .u64_counter("audio_stream_stall_total")
            .with_description("Audio streams that went quiet for the jitter buffer's stall timeout")
            .build()
    })
}

fn injected() -> &'static Counter<u64> {
    INJECTED.get_or_init(|| {
        meter()
            .u64_counter("audio_probe_injected_drops_total")
            .with_description("Frames discarded on purpose by synthetic stall injection")
            .build()
    })
}

/// Count one stall, attributed to the verdict that named its hop.
pub fn record_stall(verdict: Verdict, hops: &BTreeMap<&'static str, HopView>) {
    // `hops` is deliberately unused for attributes: a hop name would multiply
    // the series by a per-stream dimension the log line already carries.
    let _ = hops;
    stalls().add(1, &[KeyValue::new("verdict", verdict.as_str().to_string())]);
}

/// Count frames the synthetic injection discarded.
pub fn record_injected_drop(stage: Stage, frames: u64) {
    if frames == 0 {
        return;
    }
    injected().add(
        frames,
        &[KeyValue::new("stage", stage.as_str().to_string())],
    );
}
