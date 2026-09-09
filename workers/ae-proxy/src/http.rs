//! `/healthz` and `/metrics`. UDP cannot be probed, so this is what the kubelet
//! checks. Health flips only once the UDP socket is bound, so a proxy that
//! cannot open its port fails the pod rather than sitting Ready and relaying
//! nothing.

use std::sync::atomic::{AtomicBool, Ordering};

use axum::response::IntoResponse;

pub static IS_HEALTHY: AtomicBool = AtomicBool::new(false);

async fn healthz() -> impl IntoResponse {
    if IS_HEALTHY.load(Ordering::Relaxed) {
        (axum::http::StatusCode::OK, "ok")
    } else {
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, "unhealthy")
    }
}

async fn metrics() -> impl IntoResponse {
    (axum::http::StatusCode::OK, crate::metrics::gather())
}

pub fn router() -> axum::Router {
    axum::Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .route("/metrics", axum::routing::get(metrics))
}

pub async fn spawn(addr: &str) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "metrics server listening");
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router()).await {
            tracing::error!(%e, "metrics server stopped");
        }
    });
    Ok(())
}
