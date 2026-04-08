mod observability;
mod server;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub use observability::{init_metrics, init_tracing};
pub use server::ServerState;

#[derive(Clone)]
pub struct TelemetryController {
    is_healthy: Arc<AtomicBool>,
}

impl TelemetryController {
    /// Marks the service as healthy, causing the /health endpoint to return 200 OK.
    pub fn healthy(&self) {
        self.is_healthy.store(true, Ordering::Relaxed);
        tracing::info!("Service marked as healthy");
    }
}

pub struct TelemetryConfig {
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    pub metrics_port: u16,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "zako3-audio-engine".to_string(),
            otlp_endpoint: None,
            metrics_port: 9090,
        }
    }
}

/// Initializes the telemetry subsystem.
///
/// This sets up:
/// - Tracing (stdout + optional OTLP)
/// - Metrics (Prometheus registry)
/// - A background HTTP server for /metrics and /health
pub async fn init(config: TelemetryConfig) -> anyhow::Result<TelemetryController> {
    // 1. Initialize Tracing
    observability::init_tracing(&config.service_name, config.otlp_endpoint.clone())?;

    // 2. Initialize Metrics
    observability::init_metrics(&config.service_name)?;

    // 3. Prepare Server State
    let is_healthy = Arc::new(AtomicBool::new(false));
    let server_state = ServerState {
        is_healthy: is_healthy.clone(),
    };

    // 4. Spawn the telemetry server in the background
    let port = config.metrics_port;
    tokio::spawn(async move {
        if let Err(e) = server::run_server(port, server_state).await {
            tracing::error!("Telemetry server failed: {}", e);
        }
    });

    Ok(TelemetryController { is_healthy })
}
