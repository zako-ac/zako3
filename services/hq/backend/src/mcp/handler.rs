//! MCP JSON-RPC POST handler and SSE stream, on the workspace `axum`.
//!
//! The POST handler resolves the bearer token from the `Authorization` header
//! and binds it to a task-local around `route_tools`, so individual tools can
//! enforce `require_user`/`require_admin`. `initialize`/`ping` are answered
//! directly; everything else routes to the tool handler. The SSE handler streams
//! server-initiated notifications (e.g. playback) to connected clients.

use std::convert::Infallible;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response as AxumResponse};
use futures_util::StreamExt;
use mcpkit::capability::{ClientCapabilities, ServerInfo};
use mcpkit::error::JsonRpcError;
use mcpkit::protocol::{Message, Request, Response};
use mcpkit::protocol_version::ProtocolVersion;
use mcpkit::server::route_tools;
use mcpkit::{Context, NoOpPeer, ServerHandler};
use tokio_stream::wrappers::BroadcastStream;

use super::McpHttpState;
use super::auth::{resolve_auth, scope};
use super::server::McpServer;

/// MCP protocol versions this server accepts.
const SUPPORTED_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

fn is_supported_version(version: Option<&str>) -> bool {
    // Lenient: clients may omit the header (e.g. on `initialize`).
    version.is_none_or(|v| SUPPORTED_VERSIONS.contains(&v))
}

/// `POST /mcp` — handle a JSON-RPC request or notification.
pub async fn mcp_post(
    State(state): State<McpHttpState>,
    headers: HeaderMap,
    body: String,
) -> AxumResponse {
    let version = headers
        .get("mcp-protocol-version")
        .and_then(|v| v.to_str().ok());
    if !is_supported_version(version) {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported MCP protocol version '{}' (supported: {})",
                version.unwrap_or("none"),
                SUPPORTED_VERSIONS.join(", ")
            ),
        )
            .into_response();
    }

    let session_id = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let auth = resolve_auth(state.server.service(), &headers).await;

    let msg: Message = match serde_json::from_str(&body) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid JSON-RPC message: {e}"),
            )
                .into_response();
        }
    };

    match msg {
        Message::Request(request) => {
            let response = scope(auth, build_response(&state.server, &request)).await;
            match serde_json::to_string(&Message::Response(response)) {
                Ok(body) => (
                    StatusCode::OK,
                    [
                        ("content-type", "application/json".to_string()),
                        ("mcp-session-id", session_id),
                    ],
                    body,
                )
                    .into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Message::Notification(_) => {
            (StatusCode::ACCEPTED, [("mcp-session-id", session_id)]).into_response()
        }
        Message::Response(_) => (
            StatusCode::BAD_REQUEST,
            "expected a request or notification",
        )
            .into_response(),
    }
}

/// `GET /mcp/sse` — server-to-client notification stream.
pub async fn mcp_sse(State(state): State<McpHttpState>) -> impl IntoResponse {
    let rx = state.sse_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        msg.ok()
            .map(|data| Ok::<Event, Infallible>(Event::default().event("message").data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn build_response(server: &McpServer, request: &Request) -> Response {
    let req_id = request.id.clone();
    let client_caps = ClientCapabilities::default();
    let server_caps = server.capabilities();
    let peer = NoOpPeer;
    let ctx = Context::new(
        &req_id,
        None,
        &client_caps,
        &server_caps,
        ProtocolVersion::LATEST,
        &peer,
    );

    let method = request.method.as_ref();
    let params = request.params.as_ref();

    match method {
        "ping" => Response::success(request.id.clone(), serde_json::json!({})),
        "initialize" => {
            let info: ServerInfo = server.server_info();
            Response::success(
                request.id.clone(),
                serde_json::json!({
                    "protocolVersion": ProtocolVersion::LATEST.as_str(),
                    "serverInfo": info,
                    "capabilities": server.capabilities(),
                }),
            )
        }
        _ => {
            if let Some(result) = route_tools(server, method, params, &ctx).await {
                match result {
                    Ok(value) => Response::success(request.id.clone(), value),
                    Err(e) => Response::error(request.id.clone(), e.into()),
                }
            } else {
                Response::error(
                    request.id.clone(),
                    JsonRpcError::method_not_found(format!("Method '{method}' not found")),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpkit::protocol::{Message, Notification};

    #[test]
    fn version_negotiation() {
        assert!(is_supported_version(None));
        assert!(is_supported_version(Some("2025-11-25")));
        assert!(!is_supported_version(Some("1999-01-01")));
    }

    #[test]
    fn notification_is_valid_jsonrpc() {
        let note = Notification::new("notifications/playback/changed");
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&Message::Notification(note)).unwrap())
                .unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "notifications/playback/changed");
        assert!(
            json.get("id").is_none(),
            "notifications must not carry an id"
        );
    }
}
