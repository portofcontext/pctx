use crate::{
    connection_pool::McpConnectionPool,
    error::RegistryError,
    events::{
        CallbackInvocationEvent, EventOutcome, McpToolCallEvent, RegistryEvent, RegistryTrace,
    },
};
use pctx_config::server::ServerConfig;
use rmcp::model::{CallToolRequestParams, JsonObject, RawContent};
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
    time::SystemTime,
};
use tracing::{debug, info, instrument, warn};

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
    /// Append-only log of every [`invoke`] call: inputs, outputs, and timing.
    ///
    /// All clones of this registry share the same log. Use [`trace`] to
    /// obtain a handle for reading or passing to other components.
    ///
    /// [`invoke`]: PctxRegistry::invoke
    /// [`trace`]: PctxRegistry::trace
    trace: RegistryTrace,
}

impl PctxRegistry {
    /// Returns every MCP server registered in this registry as a list of
    /// `(tool_names, ServerConfig)` pairs.
    ///
    /// Used by the process pool to reconstruct a registry inside a worker
    /// process that only has access to serialisable data.
    ///
    /// # Panics
    ///
    /// Panics if either internal lock is poisoned.
    pub fn mcp_servers(&self) -> Vec<(Vec<String>, ServerConfig)> {
        let actions = self.actions.read().unwrap();
        let servers = self.servers.read().unwrap();

        let mut by_server: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for action in actions.values() {
            if let RegistryAction::Mcp(mcp_id) = action {
                by_server
                    .entry(mcp_id.sever_name.clone())
                    .or_default()
                    .push(mcp_id.tool_name.clone());
            }
        }

        by_server
            .into_iter()
            .filter_map(|(server_name, tool_names)| {
                servers.get(&server_name).map(|cfg| (tool_names, cfg.clone()))
            })
            .collect()
    }

    /// Returns the IDs of every callback registered in this registry.
    ///
    /// Used by the process pool to tell a worker which callback IDs to proxy
    /// back to the parent process.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn callback_ids(&self) -> Vec<String> {
        self.actions
            .read()
            .unwrap()
            .iter()
            .filter_map(|(id, action)| {
                if matches!(action, RegistryAction::Callback(_)) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

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

    /// Returns the underlying connection pool shared by this registry.
    pub fn pool(&self) -> Arc<McpConnectionPool> {
        self.connection_pool.clone()
    }

    /// Returns the trace shared by all clones of this registry.
    pub fn trace(&self) -> &RegistryTrace {
        &self.trace
    }

    /// Eagerly opens connections to all registered MCP servers.
    ///
    /// Iterates every server config currently held in the registry and calls
    /// [`McpConnectionPool::get_or_connect`] so that the connections are ready
    /// before the first tool call arrives. Failures are logged as warnings
    /// rather than returned as errors so that a single unreachable server does
    /// not block the others from warming up.
    pub async fn prewarm_pool(&self) -> Result<(), RegistryError> {
        let server_configs = self
            .servers
            .read()
            .map_err(|e| {
                RegistryError::Config(format!(
                    "Failed obtaining write lock on MCP server registry: {e}"
                ))
            })?
            .clone();

        for server in server_configs.values() {
            match self.connection_pool.get_or_connect(server).await {
                Ok(_) => debug!("pool: pre-warmed connection to \"{}\"", &server.name),
                Err(e) => warn!("pool: pre-warm failed for \"{}\": {e}", &server.name),
            };
        }

        Ok(())
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
                let args_json = args.as_ref().map(|a| json!(a));
                let started_at = SystemTime::now();

                let result = callback_fn(args.map(|a| json!(a))).await.map_err(|e| {
                    RegistryError::ExecutionError(format!(
                        "Failed calling callback with id \"{id}\": {e}",
                    ))
                });

                self.trace
                    .push(RegistryEvent::CallbackInvocation(CallbackInvocationEvent {
                        id: id.to_string(),
                        args: args_json,
                        outcome: match &result {
                            Ok(v) => EventOutcome::Success { output: v.clone() },
                            Err(e) => EventOutcome::Error {
                                message: e.to_string(),
                            },
                        },
                        started_at,
                        ended_at: SystemTime::now(),
                    }));

                result
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

                let args_json = args.as_ref().map(|a| json!(a));
                let started_at = SystemTime::now();

                let (client, cached_client) = self
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
                });

                let result = (|| -> Result<serde_json::Value, RegistryError> {
                    let tool_result = tool_result?;

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
                })();

                self.trace
                    .push(RegistryEvent::McpToolCall(McpToolCallEvent {
                        server: mcp_id.sever_name.clone(),
                        tool: mcp_id.tool_name.clone(),
                        cached_client,
                        args: args_json,
                        outcome: match &result {
                            Ok(v) => EventOutcome::Success { output: v.clone() },
                            Err(e) => EventOutcome::Error {
                                message: e.to_string(),
                            },
                        },
                        started_at,
                        ended_at: SystemTime::now(),
                    }));

                result
            }
        }
    }
}

impl std::fmt::Debug for PctxRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let actions: Vec<String> = match self.actions.read() {
            Ok(a) => a.keys().cloned().collect(),
            Err(_) => vec!["<locked>".to_string()],
        };
        let servers: Vec<String> = match self.servers.read() {
            Ok(s) => s.keys().cloned().collect(),
            Err(_) => vec!["<locked>".to_string()],
        };
        f.debug_struct("PctxRegistry")
            .field("actions", &actions)
            .field("servers", &servers)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for PctxRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let actions = self.actions.read();
        let servers = self.servers.read();
        let action_count = actions.as_ref().map(|a| a.len()).unwrap_or(0);
        let server_count = servers.as_ref().map(|s| s.len()).unwrap_or(0);
        write!(
            f,
            "PctxRegistry({action_count} actions, {server_count} servers)"
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::PctxRegistry;

    fn noop_callback() -> super::CallbackFn {
        Arc::new(|_args| Box::pin(async { Ok(json!(null)) }))
    }

    // ── callback_ids() ───────────────────────────────────────────────────────

    #[test]
    fn callback_ids_empty_when_no_callbacks_registered() {
        let registry = PctxRegistry::default();
        assert!(registry.callback_ids().is_empty());
    }

    #[test]
    fn callback_ids_returns_all_registered_ids() {
        let registry = PctxRegistry::default();
        registry.add_callback("ns.foo", noop_callback()).unwrap();
        registry.add_callback("ns.bar", noop_callback()).unwrap();

        let mut ids = registry.callback_ids();
        ids.sort();
        assert_eq!(ids, vec!["ns.bar", "ns.foo"]);
    }

    #[test]
    fn callback_ids_excludes_mcp_actions() {
        // An MCP action registered via add_mcp should NOT appear in callback_ids.
        use pctx_config::server::{HttpServerConfig, ServerConfig, ServerTransport};
        let registry = PctxRegistry::default();
        registry
            .add_mcp(
                &["my_tool".to_string()],
                ServerConfig {
                    name: "my_server".to_string(),
                    transport: ServerTransport::Http(HttpServerConfig {
                        url: "http://localhost:1234".parse().unwrap(),
                        auth: None,
                    }),
                },
            )
            .unwrap();
        registry.add_callback("cb.one", noop_callback()).unwrap();

        let ids = registry.callback_ids();
        assert_eq!(ids, vec!["cb.one"]);
    }

    // ── mcp_servers() ────────────────────────────────────────────────────────

    #[test]
    fn mcp_servers_empty_when_no_servers_registered() {
        let registry = PctxRegistry::default();
        assert!(registry.mcp_servers().is_empty());
    }

    #[test]
    fn mcp_servers_excludes_callbacks() {
        let registry = PctxRegistry::default();
        registry.add_callback("cb.one", noop_callback()).unwrap();
        assert!(registry.mcp_servers().is_empty());
    }

    #[test]
    fn mcp_servers_returns_tool_names_and_config() {
        use pctx_config::server::{HttpServerConfig, ServerConfig, ServerTransport};
        let registry = PctxRegistry::default();
        let cfg = ServerConfig {
            name: "svc".to_string(),
            transport: ServerTransport::Http(HttpServerConfig {
                url: "http://localhost:9999".parse().unwrap(),
                auth: None,
            }),
        };
        registry
            .add_mcp(&["tool_a".to_string(), "tool_b".to_string()], cfg.clone())
            .unwrap();

        let servers = registry.mcp_servers();
        assert_eq!(servers.len(), 1);

        let (mut tool_names, returned_cfg) = servers.into_iter().next().unwrap();
        tool_names.sort();
        assert_eq!(tool_names, vec!["tool_a", "tool_b"]);
        assert_eq!(returned_cfg.name, cfg.name);
    }
}
