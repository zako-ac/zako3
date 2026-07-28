pub mod auth;
pub mod entry;
pub mod gc;
pub mod preload;
pub mod state;
pub mod stream;
pub mod warmup;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
};
use dashmap::DashMap;
use tokio::net::TcpListener;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use zako3_preload_cache::{AudioPreload, FileAudioCache};

pub use state::AppState;
pub use warmup::WarmupState;

pub fn build(
    cache: Arc<FileAudioCache>,
    preload: Arc<AudioPreload>,
    admin_token: Option<String>,
    warmup: Arc<WarmupState>,
) -> Router {
    let state = AppState {
        cache,
        preload,
        sessions: Arc::new(DashMap::new()),
        active_by_key: Arc::new(DashMap::new()),
        admin_token,
        warmup,
    };

    // Merged after the auth layer so the probes stay unauthenticated: `.layer`
    // only wraps routes registered before it, and the kubelet has no token.
    let probes = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(readyz));

    Router::new()
        .route("/preload", post(preload::create))
        .route("/preload/:id/frames", post(preload::frames))
        .route("/preload/:id/commit", post(preload::commit))
        .route("/preload/:id/abort", post(preload::abort))
        .route("/stream", get(stream::stream))
        .route("/entry", get(entry::get_entry).delete(entry::delete_entry))
        .route("/entries", delete(entry::delete_entries))
        .route("/metadata", post(entry::store_metadata))
        .layer(middleware::from_fn_with_state(state.clone(), auth::admin_token))
        .merge(probes)
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}

/// Reports whether the on-disk index has finished loading. Informational only —
/// the readiness probe stays on `/healthz`, because the cache is meant to take
/// traffic while warming.
async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let (scanned, total) = state.warmup.progress();
    let ready = state.warmup.is_ready();
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "ready": ready,
            "scanned": scanned,
            "total": total,
        })),
    )
}

/// Open the listening socket. Kept separate from [`serve_on`] so startup can bind
/// before doing any expensive work.
pub async fn bind(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("cache server listening on {}", listener.local_addr()?);
    Ok(listener)
}

pub async fn serve_on(listener: TcpListener, router: Router) -> anyhow::Result<()> {
    axum::serve(listener, router).await?;
    Ok(())
}
