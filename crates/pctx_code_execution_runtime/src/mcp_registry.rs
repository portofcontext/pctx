use crate::error::McpError;
use pctx_config::server::ServerConfig;
use rmcp::model::{CallToolRequestParams, JsonObject, RawContent};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, instrument, warn};

/// Singleton registry for MCP server configurations
#[derive(Clone)]
pub struct MCPRegistry {
    configs: Arc<RwLock<HashMap<String, ServerConfig>>>,
}

impl MCPRegistry {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an MCP server configuration
    ///
    /// # Panics
    ///
    /// # Errors
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn add(&self, cfg: ServerConfig) -> Result<(), McpError> {
        let mut configs = self.configs.write().unwrap();

        if configs.contains_key(&cfg.name) {
            return Err(McpError::Config(format!(
                "MCP Server with name \"{}\" is already registered, you cannot register two MCP servers with the same name",
                cfg.name
            )));
        }

        configs.insert(cfg.name.clone(), cfg);
        Ok(())
    }

    /// Get an MCP server configuration by name
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn get(&self, name: &str) -> Option<ServerConfig> {
        let configs = self.configs.read().unwrap();
        configs.get(name).cloned()
    }

    /// Check if an MCP server is registered
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn has(&self, name: &str) -> bool {
        let configs = self.configs.read().unwrap();
        configs.contains_key(name)
    }

    /// Delete an MCP server configuration
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn delete(&self, name: &str) -> bool {
        let mut configs = self.configs.write().unwrap();
        configs.remove(name).is_some()
    }

    /// Clear all MCP server configurations
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn clear(&self) {
        let mut configs = self.configs.write().unwrap();
        configs.clear();
    }
}

impl Default for MCPRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Call an MCP tool on a registered server
#[instrument(
    name = "invoke_mcp_tool",
    skip_all,
    fields(id=format!("{server_name}.{tool_name}"), args = json!(args).to_string()),
    ret(Display),
    err
)]
pub(crate) async fn call_mcp_tool(
    registry: &MCPRegistry,
    server_name: &str,
    tool_name: &str,
    args: Option<JsonObject>,
) -> Result<serde_json::Value, McpError> {
    // Get the server config from registry
    let mcp_cfg = registry.get(server_name).ok_or_else(|| {
        McpError::ToolCall(format!(
            "MCP Server with name \"{server_name}\" does not exist"
        ))
    })?;

    let client = match mcp_cfg.connect().await {
        Ok(client) => client,
        Err(err) => {
            warn!(
                server = %server_name,
                error = %err,
                "Could not connect to MCP: initialization failure"
            );
            return Err(McpError::Connection(err.to_string()));
        }
    };
    let tool_result = client
        .call_tool(CallToolRequestParams {
            name: tool_name.to_string().into(),
            arguments: args,
            task: None,
            meta: None,
        })
        .await
        .map_err(|e| {
            McpError::ToolCall(format!(
                "Tool call \"{server_name}.{tool_name}\" failed: {e}"
            ))
        })?;
    let _ = client.cancel().await;

    // Check if the tool call resulted in an error
    if tool_result.is_error.unwrap_or(false) {
        return Err(McpError::ToolCall(format!(
            "Tool call \"{server_name}.{tool_name}\" failed"
        )));
    }

    // Prefer structuredContent if available, otherwise use content array
    let has_structured = tool_result.structured_content.is_some();
    let val = if let Some(structured) = tool_result.structured_content {
        structured
    } else if let Some(RawContent::Text(text_content)) = tool_result.content.first().map(|a| &**a) {
        // Try to parse as JSON, fallback to string value
        serde_json::from_str(&text_content.text)
            .or_else(|_| Ok(serde_json::Value::String(text_content.text.clone())))
            .map_err(|e: serde_json::Error| {
                McpError::ToolCall(format!("Failed to parse content: {e}"))
            })?
    } else {
        // Return the whole content array as JSON
        json!(tool_result.content)
    };

    info!(structured_content = has_structured, result =? &val, "Tool result");

    Ok(val)
}
