use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::{Resource, propagation::TraceContextPropagator, trace::SdkTracerProvider};
use tonic::metadata::{Ascii, MetadataKey, MetadataMap, MetadataValue};
use tracing_subscriber::{
    EnvFilter, Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};

pub(crate) fn build_metadata(headers_env: Option<String>) -> MetadataMap {
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

/// A boolean-ish env var: set and not `0`/`false`/`no`/empty.
fn truthy_env(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => false,
    }
}

pub fn init_tracing(service_name: &str, otlp_endpoint: Option<String>) -> anyhow::Result<()> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Route panics through tracing (and the OTLP log bridge) so a panic in any
    // spawned task surfaces in collected logs instead of silently vanishing
    // (the default hook only writes to stderr, which k8s does not gather).
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ::tracing::error!(target: "panic", "PANIC: {}", info);
        default_hook(info);
    }));

    // `protofish3` debug logging is a diagnosis-only switch: it is verbose
    // enough that leaving it on for normal traffic is its own problem. It is
    // turned on for an investigation window with one env var rather than by
    // editing RUST_LOG, so the rest of the filter stays exactly as deployed.
    let mut filter_directives = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    if truthy_env("ZAKO_PROTOFISH3_DEBUG") {
        if !filter_directives.trim().is_empty() {
            filter_directives.push(',');
        }
        filter_directives.push_str("protofish3=debug");
        tracing::info!("ZAKO_PROTOFISH3_DEBUG is set: protofish3 debug logging enabled");
    }
    let console_filter = EnvFilter::new(filter_directives);
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .compact()
        .with_filter(console_filter);
    let registry = Registry::default().with(fmt_layer);

    if let Some(endpoint) = otlp_endpoint {
        let headers_env = std::env::var("OTEL_EXPORTER_OTLP_HEADERS").ok();
        let otel_filter = std::env::var("OTEL_FILTER")
            .unwrap_or_else(|_| "debug,h2=off,tonic=off,hyper=off,tower=off,quinn=off,jsonrpsee=off,serenity::gateway=off".to_string());
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
            .with_filter(EnvFilter::new(&otel_filter));
        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(EnvFilter::new(&otel_filter));
        registry.with(otel_layer).with(log_bridge).init();
    } else {
        registry.init();
    }

    Ok(())
}
