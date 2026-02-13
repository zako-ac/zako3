use crate::metrics;
use axum::response::IntoResponse;
use std::sync::atomic::{AtomicBool, Ordering};

lazy_static::lazy_static! {
    pub static ref IS_HEALTHY: AtomicBool = AtomicBool::new(false);
}

pub async fn get_healthy() -> impl IntoResponse {
    if IS_HEALTHY.load(Ordering::Relaxed) {
        (axum::http::StatusCode::OK, "OK")
    } else {
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, "UNHEALTHY")
    }
}

pub async fn get_metrics() -> impl IntoResponse {
    let metrics = metrics::gather_metrics();
    (axum::http::StatusCode::OK, metrics)
}

pub fn create_router() -> axum::Router {
    axum::Router::new()
        .route("/healthz", axum::routing::get(get_healthy))
        .route("/metrics", axum::routing::get(get_metrics))
}

pub async fn spawn_http_server(addr: &str) {
    let app = create_router();
    tracing::info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind HTTP server");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("HTTP server failed");
    });
}
