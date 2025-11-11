//! Error types for PCTX runtime

use deno_error::{JsErrorClass, PropertyValue};
use pctx_config::server::McpConnectionError;
use std::borrow::Cow;
use std::error::Error as StdError;

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

impl JsErrorClass for McpError {
    fn get_class(&self) -> Cow<'static, str> {
        Cow::Borrowed("Error")
    }

    fn get_message(&self) -> Cow<'static, str> {
        Cow::Owned(self.to_string())
    }

    fn get_additional_properties(
        &self,
    ) -> Box<dyn Iterator<Item = (Cow<'static, str>, PropertyValue)>> {
        // No additional properties needed
        Box::new(std::iter::empty())
    }

    fn get_ref(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }
}
