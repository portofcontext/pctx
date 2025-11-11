use crate::error::McpError;
use pctx_config::server::ServerConfig;
use rmcp::model::{CallToolRequestParam, JsonObject, RawContent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Arguments for calling an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CallMCPToolArgs {
    pub name: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Option<JsonObject>,
}

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
pub(crate) async fn call_mcp_tool(
    registry: &MCPRegistry,
    args: CallMCPToolArgs,
) -> Result<serde_json::Value, McpError> {
    // Get the server config from registry
    let mcp_cfg = registry.get(&args.name).ok_or_else(|| {
        McpError::ToolCall(format!(
            "MCP Server with name \"{}\" does not exist",
            args.name
        ))
    })?;

    let client = mcp_cfg.connect().await?;
    let tool_result = client
        .call_tool(CallToolRequestParam {
            name: args.tool.clone().into(),
            arguments: args.arguments,
        })
        .await
        .unwrap();
    let _ = client.cancel().await;

    // Check if the tool call resulted in an error
    if tool_result.is_error.unwrap_or(false) {
        return Err(McpError::ToolCall(format!(
            "Tool call \"{}.{}\" failed",
            args.name, args.tool
        )));
    }

    // Prefer structuredContent if available, otherwise use content array
    if let Some(structured) = tool_result.structured_content {
        return Ok(structured);
    }

    // Convert content to JSON value
    // For simplicity, we'll extract text content and try to parse as JSON
    if let Some(RawContent::Text(text_content)) = tool_result.content.first().map(|a| &**a) {
        // Try to parse as JSON, fallback to string value
        serde_json::from_str(&text_content.text)
            .or_else(|_| Ok(serde_json::Value::String(text_content.text.clone())))
            .map_err(|e: serde_json::Error| {
                McpError::ToolCall(format!("Failed to parse content: {e}"))
            })
    } else {
        // Return the whole content array as JSON
        serde_json::to_value(&tool_result.content)
            .map_err(|e| McpError::ToolCall(format!("Failed to serialize content: {e}")))
    }
}
