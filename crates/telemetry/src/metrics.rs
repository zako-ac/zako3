use opentelemetry::KeyValue;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};

use crate::tracing::build_metadata;

/// The Prometheus exporter the services that use `zako3-telemetry` build their
/// `/metrics` endpoint on.
///
/// Kept as a named function rather than inline in [`init_metrics`] so the
/// naming rules below are testable without starting a service:
///
/// - Every counter in this workspace is already named with the Prometheus
///   convention's `_total` suffix (`hq_messages_dropped_total`,
///   `audio_track_lifecycle_total`, …), which is also the name OTLP/ClickStack
///   carries. The exporter appends a second `_total` by default, which would put
///   `hq_messages_dropped_total_total` on `/metrics` while every query and
///   runbook names the single-suffix form.
/// - Every instrument with a unit already spells it into its name
///   (`hq_http_request_duration_seconds` with `.with_unit("s")`), and the
///   exporter would append `_seconds` to that name as well.
///
/// Both suffixes are therefore turned off, so a scrape carries exactly the
/// instrument name OTLP carries.
pub fn prometheus_exporter(
    registry: prometheus::Registry,
) -> Result<opentelemetry_prometheus::PrometheusExporter, opentelemetry_sdk::error::OTelSdkError> {
    opentelemetry_prometheus::exporter()
        .with_registry(registry)
        .without_counter_suffixes()
        .without_units()
        .build()
}

pub fn init_metrics(service_name: &str, otlp_endpoint: Option<String>) -> anyhow::Result<()> {
    let registry = prometheus::default_registry();

    let prom_exporter = prometheus_exporter(registry.clone())?;

    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", service_name.to_string()))
        .build();

    let mut builder = SdkMeterProvider::builder()
        .with_reader(prom_exporter)
        .with_resource(resource);

    if let Some(endpoint) = otlp_endpoint {
        let headers_env = std::env::var("OTEL_EXPORTER_OTLP_HEADERS").ok();
        let otlp_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_metadata(build_metadata(headers_env))
            .build()?;
        let periodic_reader = PeriodicReader::builder(otlp_exporter).build();
        builder = builder.with_reader(periodic_reader);
    }

    opentelemetry::global::set_meter_provider(builder.build());

    Ok(())
}

#[cfg(test)]
mod tests {
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::SdkMeterProvider;

    use super::*;

    /// A counter named the way this workspace names them keeps exactly that
    /// name in a scrape: `/metrics` and OTLP must not disagree by a `_total`.
    #[test]
    fn a_counter_named_with_total_is_scraped_with_a_single_suffix() {
        let registry = prometheus::Registry::new();
        let exporter = prometheus_exporter(registry.clone()).expect("exporter builds");
        let provider = SdkMeterProvider::builder().with_reader(exporter).build();
        let counter = provider
            .meter("telemetry-test")
            .u64_counter("zako3_test_events_total")
            .build();
        counter.add(3, &[KeyValue::new("kind", "dropped")]);

        let families = registry.gather();
        let family = find(&families, "zako3_test_events_total");
        let sample = family.get_metric().first().expect("one sample");
        assert_eq!(sample.get_counter().value(), 3.0);
    }

    /// The same for a unit: every instrument that has one already spells it
    /// into its name, so a scrape must not repeat it (`…_seconds_seconds`).
    #[test]
    fn an_instrument_with_a_unit_is_scraped_without_a_second_unit_suffix() {
        let registry = prometheus::Registry::new();
        let exporter = prometheus_exporter(registry.clone()).expect("exporter builds");
        let provider = SdkMeterProvider::builder().with_reader(exporter).build();
        let latency = provider
            .meter("telemetry-test")
            .f64_histogram("zako3_test_latency_seconds")
            .with_unit("s")
            .build();
        latency.record(0.25, &[]);

        let families = registry.gather();
        find(&families, "zako3_test_latency_seconds");
    }

    fn find<'a>(
        families: &'a [prometheus::proto::MetricFamily],
        name: &str,
    ) -> &'a prometheus::proto::MetricFamily {
        families.iter().find(|f| f.name() == name).unwrap_or_else(|| {
            panic!(
                "`{name}` is not in the scrape; families: {:?}",
                families.iter().map(|f| f.name()).collect::<Vec<_>>()
            )
        })
    }
}
