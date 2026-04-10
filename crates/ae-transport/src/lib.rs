pub mod client;
pub mod error;
pub mod handler;
pub mod server;

mod codec;

pub use client::TlClient;
pub use error::TlError;
pub use handler::TlClientHandler;
pub use server::{TlConnectedClient, TlServer};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};
use tracing::Span;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscordToken(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TlClientHandshake {
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TlServerHandshake {
    pub token: DiscordToken,
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct WireRequest {
    pub id: u64,
    pub headers: HashMap<String, String>,
    pub payload: AudioEngineCommandRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct WireResponse {
    pub id: u64,
    pub payload: AudioEngineCommandResponse,
}

/// Injects the current tracing span ID into headers as `x-trace-id`.
pub(crate) fn inject_span(headers: &mut HashMap<String, String>) {
    let span = Span::current();
    if let Some(id) = span.id() {
        headers.insert("x-trace-id".to_string(), format!("{:x}", id.into_u64()));
    }
}

/// Creates a child span with the trace ID from the given headers.
pub(crate) fn child_span(headers: &HashMap<String, String>) -> tracing::Span {
    match headers.get("x-trace-id") {
        Some(trace_id) => tracing::info_span!("ae-transport.handle", trace_id = %trace_id),
        None => tracing::info_span!("ae-transport.handle"),
    }
}
