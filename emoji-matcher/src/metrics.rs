use prometheus::{
    Counter, Encoder, Gauge, HistogramVec, register_counter, register_gauge, register_histogram_vec,
};
use std::sync::LazyLock;

pub static EMOJI_REGISTER_REQUESTS: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!(
        "emoji_register_requests_total",
        "Total number of emoji register requests"
    )
    .unwrap()
});

pub static EMOJI_MATCH_REQUESTS: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!(
        "emoji_match_requests_total",
        "Total number of emoji match requests"
    )
    .unwrap()
});

pub static EMOJI_MATCH_HITS: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!("emoji_match_hits_total", "Total number of emoji match hits").unwrap()
});

pub static EMOJI_HASH_TIME: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!("emoji_hash_time_seconds", "Time taken to hash an emoji").unwrap()
});

pub static DB_QUERY_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "emoji_db_query_duration_seconds",
        "Latency of database queries",
        &["operation"]
    )
    .unwrap()
});

pub static EXTERNAL_FETCH_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "emoji_external_fetch_duration_seconds",
        "Latency of external image fetches",
        &["domain"]
    )
    .unwrap()
});

pub static NATS_PROCESS_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "emoji_nats_message_process_duration_seconds",
        "Time spent processing a NATS request",
        &["subject"]
    )
    .unwrap()
});

pub fn gather_metrics() -> String {
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    let encoder = prometheus::TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
