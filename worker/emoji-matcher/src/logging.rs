use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{SdkTracerProvider, Tracer},
};
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(otlp_endpoint: Option<String>) {
    // Set global propagator for context propagation
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Default filter for console logs
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Standard stdout logging layer
    let fmt_layer = fmt::layer();

    // The registry acts as the base subscriber
    let registry = Registry::default().with(env_filter).with(fmt_layer);

    if let Some(endpoint) = otlp_endpoint {
        // Initialize the OpenTelemetry tracer
        let tracer = init_tracer(&endpoint);

        // Create a tracing layer that exports to OTLP
        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        // Add the telemetry layer to the registry and initialize
        registry.with(telemetry_layer).init();
    } else {
        // If no OTLP endpoint is provided, just initialize with stdout logging
        registry.init();
    }
}

// Initialize the OTLP tracer using the tonic gRPC exporter
fn init_tracer(endpoint: &str) -> Tracer {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .expect("Failed to create OTLP exporter");

    let resource = Resource::builder()
        .with_service_name("emoji-matcher")
        .build();

    SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build()
        .tracer("emoji-matcher")
}
