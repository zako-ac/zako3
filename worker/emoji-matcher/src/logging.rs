/// Initialize tracing for the emoji-matcher service.
///
/// Delegates to the shared `zako3_telemetry` crate so all services use a
/// consistent OTLP setup. Prometheus metrics are still managed independently
/// by `crate::metrics`.
pub fn init_logging(otlp_endpoint: Option<String>) {
    zako3_telemetry::init_tracing("emoji-matcher", otlp_endpoint)
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize tracing: {e}");
        });
}
