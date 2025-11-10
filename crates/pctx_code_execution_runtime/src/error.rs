//! Error types for PCTX runtime

use deno_error::{JsErrorClass, PropertyValue};
use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;

/// Error type for MCP operations
#[derive(Debug)]
pub enum McpError {
    /// Server configuration error (e.g., duplicate name)
    ConfigError(String),
    /// Tool call error (HTTP, parsing, etc.)
    ToolCallError(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ConfigError(msg) => write!(f, "MCP configuration error: {msg}"),
            McpError::ToolCallError(msg) => write!(f, "MCP tool call error: {msg}"),
        }
    }
}

impl StdError for McpError {}

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
