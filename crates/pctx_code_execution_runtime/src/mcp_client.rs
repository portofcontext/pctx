use crate::error::McpError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub name: String,
    pub url: String,
    // TODO: Add authentication fields when needed
}

/// Arguments for calling an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CallMCPToolArgs {
    pub name: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

/// MCP JSON-RPC request (sent to server)
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: ToolCallParams,
}

/// MCP tool call parameters
#[derive(Debug, Serialize)]
struct ToolCallParams {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<serde_json::Value>,
}

/// MCP JSON-RPC response (received from server)
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    #[serde(flatten)]
    result_or_error: JsonRpcResultOrError,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsonRpcResultOrError {
    Result { result: ToolCallResult },
    Error { error: JsonRpcError },
}

#[derive(Debug, Deserialize)]
struct ToolCallResult {
    #[serde(rename = "isError")]
    is_error: Option<bool>,
    content: Option<Vec<ContentItem>>,
    #[serde(rename = "structuredContent", default)]
    structured_content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[allow(dead_code)]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ContentItem {
    Text {
        #[serde(rename = "type")]
        content_type: String,
        text: String,
    },
    Image {
        #[serde(rename = "type")]
        content_type: String,
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        #[serde(rename = "type")]
        content_type: String,
        resource: serde_json::Value,
    },
}

/// Singleton registry for MCP server configurations
#[derive(Clone)]
pub struct MCPRegistry {
    configs: Arc<RwLock<HashMap<String, MCPServerConfig>>>,
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
    pub fn add(&self, cfg: MCPServerConfig) -> Result<(), McpError> {
        let mut configs = self.configs.write().unwrap();

        if configs.contains_key(&cfg.name) {
            return Err(McpError::ConfigError(format!(
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
    pub fn get(&self, name: &str) -> Option<MCPServerConfig> {
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
        McpError::ToolCallError(format!(
            "MCP Server with name \"{}\" does not exist",
            args.name
        ))
    })?;

    // Create HTTP client
    let client = reqwest::Client::new();

    // Build the JSON-RPC request
    let request_body = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "tools/call".to_string(),
        params: ToolCallParams {
            name: args.tool.clone(),
            arguments: args.arguments,
        },
    };

    // Make the HTTP request to the MCP server
    // Using the MCP JSON-RPC 2.0 protocol
    // MCP servers require Accept header for both JSON and SSE
    let response = client
        .post(&mcp_cfg.url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| McpError::ToolCallError(format!("HTTP request failed: {e}")))?;

    // Check HTTP status
    if !response.status().is_success() {
        return Err(McpError::ToolCallError(format!(
            "HTTP request failed with status {}: {}",
            response.status(),
            response.text().await.unwrap_or_default()
        )));
    }

    // Get response body as text (could be SSE or JSON)
    let response_text = response
        .text()
        .await
        .map_err(|e| McpError::ToolCallError(format!("Failed to read response: {e}")))?;

    // Parse response - handle both SSE and plain JSON
    let json_text = if response_text.starts_with("event:") || response_text.starts_with("data:") {
        // SSE format - extract JSON from "data:" line
        response_text
            .lines()
            .find(|line| line.starts_with("data:"))
            .and_then(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .ok_or_else(|| McpError::ToolCallError("SSE response missing data line".to_string()))?
    } else {
        // Plain JSON
        response_text.trim()
    };

    // Parse JSON-RPC response
    let jsonrpc_response: JsonRpcResponse = serde_json::from_str(json_text)
        .map_err(|e| McpError::ToolCallError(format!("Failed to parse JSON response: {e}")))?;

    // Handle JSON-RPC result or error
    let tool_result = match jsonrpc_response.result_or_error {
        JsonRpcResultOrError::Error { error } => {
            return Err(McpError::ToolCallError(format!(
                "JSON-RPC error {}: {}",
                error.code, error.message
            )));
        }
        JsonRpcResultOrError::Result { result } => result,
    };

    // Check if the tool call resulted in an error
    if tool_result.is_error.unwrap_or(false) {
        return Err(McpError::ToolCallError(format!(
            "Tool call \"{}.{}\" failed",
            args.name, args.tool
        )));
    }

    // Prefer structuredContent if available, otherwise use content array
    if let Some(structured) = tool_result.structured_content {
        return Ok(structured);
    }

    // Extract content from response
    let content = tool_result.content.ok_or_else(|| {
        McpError::ToolCallError(format!(
            "Tool call \"{}.{}\" returned no content",
            args.name, args.tool
        ))
    })?;

    // Convert content to JSON value
    // For simplicity, we'll extract text content and try to parse as JSON
    if let Some(ContentItem::Text { text, .. }) = content.first() {
        // Try to parse as JSON, fallback to string value
        serde_json::from_str(text)
            .or_else(|_| Ok(serde_json::Value::String(text.clone())))
            .map_err(|e: serde_json::Error| {
                McpError::ToolCallError(format!("Failed to parse content: {e}"))
            })
    } else {
        // Return the whole content array as JSON
        serde_json::to_value(&content)
            .map_err(|e| McpError::ToolCallError(format!("Failed to serialize content: {e}")))
    }
}
