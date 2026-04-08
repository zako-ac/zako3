use opentelemetry::KeyValue;
use opentelemetry_sdk::{Resource, metrics::SdkMeterProvider};

pub fn init_metrics(service_name: &str) -> anyhow::Result<()> {
    let registry = prometheus::default_registry();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()?;

    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", service_name.to_string()))
        .build();

    let provider = SdkMeterProvider::builder()
        .with_reader(exporter)
        .with_resource(resource)
        .build();

    opentelemetry::global::set_meter_provider(provider);

    Ok(())
}
