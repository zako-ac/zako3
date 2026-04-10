use axum::{Router, routing::get};

pub(crate) async fn run_healthcheck_server(port: u16) {
    let app = Router::new().route("/health", get(|| async { "ok" }));
    match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => {
            tracing::info!("Healthcheck listening on 0.0.0.0:{port}");
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Healthcheck server error: {e}");
            }
        }
        Err(e) => tracing::error!("Failed to bind healthcheck on port {port}: {e}"),
    }
}
