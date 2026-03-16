use crate::{connection_pool::McpConnectionPool, error::RegistryError};
use pctx_config::server::ServerConfig;
use rmcp::model::{CallToolRequestParams, JsonObject, RawContent};
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};
use tracing::{info, instrument, warn};

pub type CallbackFn = Arc<
    dyn Fn(
            Option<serde_json::Value>,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct McpToolId {
    pub sever_name: String,
    pub tool_name: String,
}
impl McpToolId {
    pub fn id(&self) -> String {
        format!("{}__{}", &self.sever_name, self.tool_name)
    }
}

#[derive(Clone)]
pub enum RegistryAction {
    Mcp(McpToolId),
    Callback(CallbackFn),
}

#[derive(Clone, Default)]
pub struct PctxRegistry {
    /// All registered actions keyed by their id. An action is either an MCP
    /// tool call (identified by server + tool name) or a local Rust callback.
    actions: Arc<RwLock<HashMap<String, RegistryAction>>>,
    /// Configuration for each registered upstream MCP server, keyed by server
    /// name. Used by [`invoke`] to look up connection details when dispatching
    /// an [`RegistryAction::Mcp`] action.
    ///
    /// [`invoke`]: PctxRegistry::invoke
    servers: Arc<RwLock<HashMap<String, ServerConfig>>>,
    /// Connection pool used for upstream MCP calls. By default each registry
    /// gets its own fresh pool, so connections are scoped to the registry's
    /// lifetime. Pass a shared pool via [`with_pool`] to reuse connections
    /// across multiple registries (e.g. for a session-scoped pool).
    ///
    /// [`with_pool`]: PctxRegistry::with_pool
    connection_pool: Arc<McpConnectionPool>,
}

impl PctxRegistry {
    /// Returns the ids of this Pctx Registry.
    ///
    /// # Panics
    ///
    /// Panics if it fails acquiring the lock
    pub fn ids(&self) -> Vec<String> {
        self.actions
            .read()
            .unwrap()
            .keys()
            .map(String::from)
            .collect()
    }

    /// Replaces the registry's connection pool with the provided one.
    ///
    /// Use this to share a session-scoped pool across multiple registries so
    /// that upstream connections survive across `execute_typescript` calls.
    pub fn with_pool(mut self, pool: Arc<McpConnectionPool>) -> Self {
        self.connection_pool = pool;
        self
    }

    pub fn add_mcp(&self, tool_names: &[String], cfg: ServerConfig) -> Result<(), RegistryError> {
        // confirm unique server name
        let mut servers = self.servers.write().map_err(|e| {
            RegistryError::Config(format!(
                "Failed obtaining write lock on MCP server registry: {e}"
            ))
        })?;
        if servers.contains_key(&cfg.name) {
            return Err(RegistryError::Config(format!(
                "MCP Server with name \"{}\" is already registered, you cannot register two MCP servers with the same name",
                cfg.name
            )));
        }

        // confirm unique MCP tool ids
        let to_add: Vec<McpToolId> = tool_names
            .into_iter()
            .map(|n| McpToolId {
                sever_name: cfg.name.clone(),
                tool_name: n.clone(),
            })
            .collect();

        let mut actions = self.actions.write().map_err(|e| {
            RegistryError::Config(format!(
                "Failed obtaining write lock on action registry: {e}"
            ))
        })?;
        let already_exists: Vec<String> = to_add
            .iter()
            .filter_map(|t| {
                if actions.contains_key(&t.id()) {
                    Some(t.id())
                } else {
                    None
                }
            })
            .collect();
        if servers.contains_key(&cfg.name) {
            return Err(RegistryError::Config(format!(
                "Registry action(s) with id(s) {already_exists:?} are already registered, you cannot register two registry actions with the same id",
            )));
        }

        // register
        servers.insert(cfg.name.clone(), cfg);
        actions.extend(to_add.into_iter().map(|t| (t.id(), RegistryAction::Mcp(t))));

        Ok(())
    }

    pub fn add_callback(&self, id: &str, callback: CallbackFn) -> Result<(), RegistryError> {
        let mut actions = self.actions.write().map_err(|e| {
            RegistryError::Config(format!(
                "Failed obtaining write lock on action registry: {e}"
            ))
        })?;

        if actions.contains_key(id) {
            return Err(RegistryError::Config(format!(
                "Registry action with id {id:?} is already registered, you cannot register two registry actions with the same id",
            )));
        }

        actions.insert(id.into(), RegistryAction::Callback(callback));

        Ok(())
    }

    /// Remove an action from the registry by id
    ///
    /// # Panics
    ///
    /// Panics if cannot obtain lock
    pub fn remove(&self, id: &str) -> Option<RegistryAction> {
        let mut actions = self.actions.write().unwrap();
        actions.remove(id)
    }

    /// Get an action from the registry by id
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn get(&self, id: &str) -> Option<RegistryAction> {
        let actions = self.actions.read().unwrap();
        actions.get(id).cloned()
    }

    /// Confirms the  registry contains a given id
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn has(&self, id: &str) -> bool {
        let actions = self.actions.read().unwrap();
        actions.contains_key(id)
    }

    /// invokes the action with the provided args
    ///
    /// # Errors
    ///
    /// This function will return an error if an action by the provided id doesn't exist
    /// or if the action itself fails
    #[instrument(
        name = "invoke_registry_action",
        skip_all,
        fields(id=id, args = json!(args).to_string()),
        ret(Display),
        err
    )]
    pub async fn invoke(
        &self,
        id: &str,
        args: Option<JsonObject>,
    ) -> Result<serde_json::Value, RegistryError> {
        let action = self.get(id).ok_or_else(|| {
            RegistryError::ToolCall(format!("Action with id \"{id}\" does not exist"))
        })?;

        match action {
            RegistryAction::Callback(callback_fn) => {
                callback_fn(args.map(|a| json!(a))).await.map_err(|e| {
                    RegistryError::ExecutionError(format!(
                        "Failed calling callback with id \"{id}\": {e}",
                    ))
                })
            }

            RegistryAction::Mcp(mcp_id) => {
                let server = {
                    let servers = self.servers.read().map_err(|e| {
                        RegistryError::Config(format!(
                            "Failed obtaining read lock on MCP server registry: {e}"
                        ))
                    })?;
                    servers
                        .get(&mcp_id.sever_name)
                        .ok_or(RegistryError::ToolCall(format!(
                            "MCP server with name \"{}\" does not exist",
                            &mcp_id.sever_name
                        )))?
                        .clone()
                };

                let client = self
                    .connection_pool
                    .get_or_connect(&server)
                    .await
                    .map_err(|err| {
                        warn!(
                            server = %mcp_id.sever_name,
                            error = %err,
                            "Could not connect to MCP: initialization failure"
                        );
                        err
                    })?;

                let mut params = CallToolRequestParams::new(mcp_id.tool_name.to_string());
                if let Some(args) = args {
                    params = params.with_arguments(args);
                }
                let tool_result = client.call_tool(params).await.map_err(|e| {
                    RegistryError::ToolCall(format!("Tool call \"{}\" failed: {e}", mcp_id.id()))
                })?;

                // Check if the tool call resulted in an error
                if tool_result.is_error.unwrap_or(false) {
                    return Err(RegistryError::ToolCall(format!(
                        "Tool call \"{}\" failed",
                        mcp_id.id()
                    )));
                }

                // Prefer structuredContent if available, otherwise use content array
                let has_structured = tool_result.structured_content.is_some();
                let val = if let Some(structured) = tool_result.structured_content {
                    structured
                } else if let Some(RawContent::Text(text_content)) =
                    tool_result.content.first().map(|a| &**a)
                {
                    // Try to parse as JSON, fallback to string value
                    serde_json::from_str(&text_content.text)
                        .or_else(|_| Ok(serde_json::Value::String(text_content.text.clone())))
                        .map_err(|e: serde_json::Error| {
                            RegistryError::ToolCall(format!("Failed to parse content: {e}"))
                        })?
                } else {
                    // Return the whole content array as JSON
                    json!(tool_result.content)
                };

                info!(structured_content = has_structured, result =? &val, "Tool result");

                Ok(val)
            }
        }
    }
}
