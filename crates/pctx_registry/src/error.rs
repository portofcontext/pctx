use pctx_config::server::McpConnectionError;

/// Error type for registry operations
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// Configuration error (e.g., duplicate name)
    #[error("Registry configuration error: {0}")]
    Config(String),
    /// Server connection error
    #[error("MCP connection error: {0}")]
    Connection(String),
    /// Tool call error (HTTP, parsing, etc.)
    #[error("Tool call error: {0}")]
    ToolCall(String),
    /// Callback execution error
    #[error("Callback execution error: {0}")]
    ExecutionError(String),
}

impl From<McpConnectionError> for RegistryError {
    fn from(value: McpConnectionError) -> Self {
        Self::Connection(value.to_string())
    }
}

impl deno_error::JsErrorClass for RegistryError {
    fn get_class(&self) -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Error")
    }

    fn get_message(&self) -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Owned(self.to_string())
    }

    fn get_additional_properties(
        &self,
    ) -> Box<dyn Iterator<Item = (std::borrow::Cow<'static, str>, deno_error::PropertyValue)>> {
        Box::new(std::iter::empty())
    }

    fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }
}
