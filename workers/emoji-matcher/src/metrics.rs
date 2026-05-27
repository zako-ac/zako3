use prometheus::{Counter, Encoder, Gauge, register_counter, register_gauge};
use std::sync::LazyLock;

pub static EMOJI_SCOPE_MATCH_REQUESTS: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!(
        "emoji_scope_match_requests_total",
        "Total number of scope-match requests received"
    )
    .unwrap()
});

pub static EMOJI_SCOPE_MATCH_DROPS: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!(
        "emoji_scope_match_drops_total",
        "Scope-match requests dropped because the task queue was full"
    )
    .unwrap()
});

pub static EMOJI_SCOPE_MATCH_HITS: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!(
        "emoji_scope_match_hits_total",
        "Scope-match requests that resulted in a new mapping being written"
    )
    .unwrap()
});

pub static EMOJI_HASH_TIME: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!("emoji_hash_time_seconds", "Time taken to hash an emoji").unwrap()
});

pub fn gather_metrics() -> String {
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    let encoder = prometheus::TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
