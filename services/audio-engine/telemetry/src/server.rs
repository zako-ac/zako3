use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{Router, extract::State, http::StatusCode, routing::get};
use prometheus::{Encoder, TextEncoder};

#[derive(Clone)]
pub struct ServerState {
    pub is_healthy: Arc<AtomicBool>,
}

pub async fn run_server(port: u16, state: ServerState) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Telemetry server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler(State(state): State<ServerState>) -> StatusCode {
    if state.is_healthy.load(Ordering::Relaxed) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];

    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return String::new();
    }

    String::from_utf8(buffer).unwrap_or_default()
}
