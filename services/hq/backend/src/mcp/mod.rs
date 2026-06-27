//! MCP (Model Context Protocol) surface for the HQ backend.
//!
//! Exposes every REST data/action endpoint as an MCP tool over a small
//! Streamable-HTTP transport implemented on the workspace `axum`, plus an SSE
//! channel that pushes playback notifications. Mounted into the main backend
//! app at `/mcp` (POST) and `/mcp/sse` (GET) by [`mcp_routes`].
//!
//! We use the `mcpkit` crate (core/server) for the protocol types, `ToolService`
//! and `route_tools`, but implement the HTTP/SSE transport here rather than via
//! `mcpkit-axum` (which pins an incompatible `axum` version).

pub mod auth;
pub mod handler;
pub mod notifier;
pub mod server;
pub mod support;
pub mod tools;

pub use server::McpServer;

use axum::Router;
use axum::routing::{get, post};
use hq_core::{PlaybackEvent, Service};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

/// Shared state for the MCP HTTP/SSE handlers.
#[derive(Clone)]
pub struct McpHttpState {
    pub server: Arc<McpServer>,
    /// Broadcast channel feeding all connected `/mcp/sse` clients.
    pub sse_tx: broadcast::Sender<String>,
}

/// Build the MCP sub-router (`/mcp`, `/mcp/sse`) and spawn the playback
/// notifier. Returns a finalized `Router` (state erased) for merging into the
/// main backend app.
pub fn mcp_routes(service: Arc<Service>, event_tx: broadcast::Sender<PlaybackEvent>) -> Router {
    let server = Arc::new(McpServer::new(service));
    let (sse_tx, _) = broadcast::channel::<String>(128);

    notifier::spawn_playback_notifier(sse_tx.clone(), event_tx);

    let state = McpHttpState { server, sse_tx };

    Router::new()
        .route("/mcp", post(handler::mcp_post))
        .route("/mcp/sse", get(handler::mcp_sse))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
