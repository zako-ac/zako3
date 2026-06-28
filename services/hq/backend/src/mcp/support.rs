//! Shared helpers for MCP tool handlers.
//!
//! Tools return `Result<ToolOutput, McpError>`. We model *domain* failures
//! (auth, validation, not-found) as `ToolOutput::error` (an `isError` tool
//! result the client/model can see) and reserve `McpError` for protocol-level
//! failures. To keep tool bodies flat, each tool runs an inner future that
//! yields `Result<ToolOutput, ToolOutput>` and [`run`] collapses it.

use hq_core::CoreError;
use mcpkit::error::McpError;
use mcpkit::server::capability::tools::ToolBuilder;
use mcpkit::types::{Tool, ToolOutput};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Collapse an inner `Result<ToolOutput, ToolOutput>` (Err = error output) into
/// the `Result<ToolOutput, McpError>` shape the tool handler trait expects.
pub(crate) async fn run<F>(f: F) -> Result<ToolOutput, McpError>
where
    F: std::future::Future<Output = Result<ToolOutput, ToolOutput>> + Send,
{
    Ok(f.await.unwrap_or_else(|e| e))
}

/// Deserialize tool arguments into `T`, surfacing parse errors as a tool error.
pub(crate) fn parse_args<T: DeserializeOwned>(args: serde_json::Value) -> Result<T, ToolOutput> {
    serde_json::from_value(args).map_err(|e| ToolOutput::error(format!("invalid arguments: {e}")))
}

/// Serialize a value to a JSON text tool output.
pub(crate) fn json_ok<T: Serialize>(v: &T) -> Result<ToolOutput, ToolOutput> {
    match serde_json::to_string(v) {
        Ok(s) => Ok(ToolOutput::text(s)),
        Err(e) => Err(ToolOutput::error(format!("serialization error: {e}"))),
    }
}

/// A plain-text success output (for void endpoints).
pub(crate) fn text_ok(s: impl Into<String>) -> Result<ToolOutput, ToolOutput> {
    Ok(ToolOutput::text(s.into()))
}

/// Map a core service error to a tool error output (mirrors handler `map_error`).
pub(crate) fn map_core(e: CoreError) -> ToolOutput {
    ToolOutput::error(e.to_string())
}

/// Build a tool definition with a name, description and JSON input schema.
pub(crate) fn mk_tool(name: &str, desc: &str, schema: serde_json::Value) -> Tool {
    ToolBuilder::new(name)
        .description(desc)
        .input_schema(schema)
        .build()
}
