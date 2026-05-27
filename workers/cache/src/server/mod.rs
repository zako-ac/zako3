pub mod auth;
pub mod entry;
pub mod gc;
pub mod preload;
pub mod state;
pub mod stream;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router, middleware,
    routing::{get, post},
};
use dashmap::DashMap;
use tokio::net::TcpListener;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use zako3_preload_cache::{AudioPreload, FileAudioCache};

pub use state::AppState;

pub fn build(
    cache: Arc<FileAudioCache>,
    preload: Arc<AudioPreload>,
    admin_token: Option<String>,
) -> Router {
    let state = AppState {
        cache,
        preload,
        sessions: Arc::new(DashMap::new()),
        active_by_key: Arc::new(DashMap::new()),
        admin_token,
    };

    Router::new()
        .route("/preload", post(preload::create))
        .route("/preload/:id/frames", post(preload::frames))
        .route("/preload/:id/commit", post(preload::commit))
        .route("/preload/:id/abort", post(preload::abort))
        .route("/stream", get(stream::stream))
        .route("/entry", get(entry::get_entry).delete(entry::delete_entry))
        .route("/metadata", post(entry::store_metadata))
        .route("/healthz", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state.clone(), auth::admin_token))
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, router: Router) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("cache server listening on {}", listener.local_addr()?);
    axum::serve(listener, router).await?;
    Ok(())
}
