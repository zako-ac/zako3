use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use tonic::metadata::{Ascii, MetadataKey, MetadataMap, MetadataValue};
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt};

fn build_metadata(headers_env: Option<String>) -> MetadataMap {
    let mut metadata = MetadataMap::new();
    if let Some(headers_str) = headers_env {
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
    metadata
}

pub fn init_tracing(service_name: &str, otlp_endpoint: Option<String>) -> anyhow::Result<()> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_target(true).with_level(true).compact().with_filter(console_filter);
    let registry = Registry::default().with(fmt_layer);

    if let Some(endpoint) = otlp_endpoint {
        let headers_env = std::env::var("OTEL_EXPORTER_OTLP_HEADERS").ok();
        let resource = Resource::builder()
            .with_service_name(service_name.to_string())
            .build();

        // Traces
        let span_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint.clone())
            .with_metadata(build_metadata(headers_env.clone()))
            .build()?;
        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(span_exporter)
            .with_resource(resource.clone())
            .build();
        let tracer = tracer_provider.tracer(service_name.to_string());
        global::set_tracer_provider(tracer_provider);

        // Logs
        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_metadata(build_metadata(headers_env))
            .build()?;
        let logger_provider = SdkLoggerProvider::builder()
            .with_batch_exporter(log_exporter)
            .with_resource(resource)
            .build();
        let log_bridge = OpenTelemetryTracingBridge::new(&logger_provider)
            .with_filter(EnvFilter::new("debug,h2=off,tonic=off,hyper=off,tower=off"));
        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(EnvFilter::new("debug,h2=off,tonic=off,hyper=off,tower=off"));
        registry.with(otel_layer).with(log_bridge).init();
    } else {
        registry.init();
    }

    Ok(())
}
