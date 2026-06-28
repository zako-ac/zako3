//! The MCP server handler exposing the HQ API as tools.
//!
//! `McpServer` holds the shared `Service` plus a `ToolService` populated with
//! one tool per REST operation. It implements `ServerHandler` (info +
//! capabilities) and delegates `ToolHandler` to the inner `ToolService`. The
//! `#[mcp_server]` macro is intentionally not used: our request DTOs derive
//! serde (not `schemars::JsonSchema`), so we register tools with explicit JSON
//! schemas and `serde_json` deserialization instead.

use hq_core::Service;
use mcpkit::capability::{ServerCapabilities, ServerInfo};
use mcpkit::error::McpError;
use mcpkit::server::capability::tools::ToolService;
use mcpkit::types::{Tool, ToolOutput};
use mcpkit::{Context, ServerHandler, ToolHandler};
use serde_json::Value;
use std::sync::Arc;

pub struct McpServer {
    service: Arc<Service>,
    tools: ToolService,
}

impl McpServer {
    pub fn new(service: Arc<Service>) -> Self {
        let mut tools = ToolService::new();
        crate::mcp::tools::register_all(&mut tools, &service);
        Self { service, tools }
    }

    /// The shared service, used by the POST handler to resolve bearer auth.
    pub(crate) fn service(&self) -> &Arc<Service> {
        &self.service
    }
}

impl ServerHandler for McpServer {
    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            name: "hq".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: None,
        }
    }

    fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities::new().with_tools()
    }

    fn instructions(&self) -> Option<String> {
        Some(
            "HQ control-plane API exposed as MCP tools. Authenticate by sending an \
             'Authorization: Bearer <JWT>' header; admin tools additionally require an \
             admin account. Subscribe to /mcp/sse for playback notifications."
                .to_string(),
        )
    }
}

impl ToolHandler for McpServer {
    async fn list_tools(&self, ctx: &Context<'_>) -> Result<Vec<Tool>, McpError> {
        self.tools.list_tools(ctx).await
    }

    async fn call_tool(
        &self,
        name: &str,
        args: Value,
        ctx: &Context<'_>,
    ) -> Result<ToolOutput, McpError> {
        self.tools.call_tool(name, args, ctx).await
    }
}
