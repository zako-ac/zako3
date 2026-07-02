use opentelemetry::KeyValue;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};

use crate::tracing::build_metadata;

pub fn init_metrics(service_name: &str, otlp_endpoint: Option<String>) -> anyhow::Result<()> {
    let registry = prometheus::default_registry();

    let prom_exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()?;

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
