mod metrics;
mod server;
mod tracing;

pub use metrics::init_metrics;
pub use server::ServerState;
pub use tracing::init_tracing;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct TelemetryController {
    is_healthy: Arc<AtomicBool>,
}

impl TelemetryController {
    /// Marks the service as healthy, causing the /health endpoint to return 200 OK.
    pub fn healthy(&self) {
        self.is_healthy.store(true, Ordering::Relaxed);
        ::tracing::info!("Service marked as healthy");
    }
}

pub struct TelemetryConfig {
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    /// Port for the /health and /metrics HTTP server. `None` skips spawning it.
    pub metrics_port: Option<u16>,
}

impl TelemetryConfig {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            otlp_endpoint: None,
            metrics_port: None,
        }
    }
}

/// Initializes the telemetry subsystem for a service.
///
/// Sets up:
/// - Structured stdout logging (respects `RUST_LOG`)
/// - Optional OTLP trace export (when `otlp_endpoint` is set)
/// - Prometheus metrics via the global OTel meter provider
/// - Optional HTTP server on `metrics_port` serving `/health` and `/metrics`
pub async fn init(config: TelemetryConfig) -> anyhow::Result<TelemetryController> {
    tracing::init_tracing(&config.service_name, config.otlp_endpoint.clone())?;
    metrics::init_metrics(&config.service_name, config.otlp_endpoint.clone())?;

    let is_healthy = Arc::new(AtomicBool::new(false));

    if let Some(port) = config.metrics_port {
        let server_state = ServerState {
            is_healthy: is_healthy.clone(),
        };
        tokio::spawn(async move {
            if let Err(e) = server::run_server(port, server_state).await {
                ::tracing::error!("Telemetry server failed: {}", e);
            }
        });
    }

    Ok(TelemetryController { is_healthy })
}
