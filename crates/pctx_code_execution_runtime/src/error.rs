//! Error types for PCTX runtime

use pctx_config::server::McpConnectionError;

/// Error type for MCP operations
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Server configuration error (e.g., duplicate name)
    #[error("MCP configuration error: {0}")]
    Config(String),
    /// Server connection error
    #[error("MCP connection error: {0}")]
    Connection(String),
    /// Tool call error (HTTP, parsing, etc.)
    #[error("MCP tool call error: {0}")]
    ToolCall(String),
}

impl From<McpConnectionError> for McpError {
    fn from(value: McpConnectionError) -> Self {
        Self::Connection(value.to_string())
    }
}

// Use the shared macro for JsErrorClass implementation
crate::impl_js_error_class!(McpError);
