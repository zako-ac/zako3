use prometheus::{Encoder, IntCounterVec, IntGauge, register_int_counter_vec, register_int_gauge};
use std::sync::LazyLock;

pub static FORWARDED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "aeproxy_datagrams_forwarded_total",
        "Datagrams relayed, by direction",
        &["direction"]
    )
    .unwrap()
});

pub static DROPPED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "aeproxy_datagrams_dropped_total",
        "Datagrams not relayed, by reason",
        &["reason"]
    )
    .unwrap()
});

pub static LOOKUPS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "aeproxy_route_lookups_total",
        "Route lookups against Redis, by result",
        &["result"]
    )
    .unwrap()
});

/// Eviction is what actually bounds the table — HQ's routes carry a one-hour
/// TTL and are not always deleted on completion — so a rising rate here is the
/// signal that the cap is too low, not noise.
pub static EVICTED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "aeproxy_routes_evicted_total",
        "Routes removed by the sweep, by reason",
        &["reason"]
    )
    .unwrap()
});

pub static ROUTES: LazyLock<IntGauge> =
    LazyLock::new(|| register_int_gauge!("aeproxy_routes", "Live routes in the table").unwrap());

pub fn gather() -> String {
    let families = prometheus::gather();
    let mut buf = Vec::new();
    prometheus::TextEncoder::new().encode(&families, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}
