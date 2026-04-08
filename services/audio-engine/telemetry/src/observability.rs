use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use tonic::metadata::{Ascii, MetadataKey, MetadataMap, MetadataValue};
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing(service_name: &str, otlp_endpoint: Option<String>) -> anyhow::Result<()> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .compact();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = Registry::default().with(env_filter).with(fmt_layer);

    if let Some(endpoint) = otlp_endpoint {
        let mut metadata = MetadataMap::new();
        if let Ok(headers_str) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
            for pair in headers_str.split(',') {
                if let Some((k, v)) = pair.split_once('=') {
                    if let (Ok(key), Ok(val)) = (
                        k.trim().to_lowercase().parse::<MetadataKey<Ascii>>(),
                        v.trim().parse::<MetadataValue<Ascii>>(),
                    ) {
                        metadata.insert(key, val);
                    }
                }
            }
        }

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_metadata(metadata)
            .build()?;

        let resource = Resource::builder()
            .with_service_name(service_name.to_string())
            .build();

        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();

        let tracer = provider.tracer(service_name.to_string());
        global::set_tracer_provider(provider);

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        registry.with(otel_layer).init();
    } else {
        registry.init();
    }

    Ok(())
}

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

    global::set_meter_provider(provider);

    Ok(())
}
