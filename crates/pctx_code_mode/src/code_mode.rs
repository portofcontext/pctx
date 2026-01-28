use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use pctx_code_execution_runtime::CallbackRegistry;
use pctx_codegen::{Tool, ToolSet};
use pctx_config::server::ServerConfig;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, instrument, warn};

use crate::{
    Error, Result,
    model::{
        CallbackConfig, ExecuteOutput, FunctionDetails, GetFunctionDetailsInput,
        GetFunctionDetailsOutput, ListFunctionsOutput, ListedFunction,
    },
};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CodeMode {
    // Codegen interfaces
    tool_sets: Vec<pctx_codegen::ToolSet>,

    // configurations
    servers: Vec<ServerConfig>,
    callbacks: Vec<CallbackConfig>,
}

impl CodeMode {
    // --------------- Builder functions ---------------

    pub async fn with_server(mut self, server: &ServerConfig) -> Result<Self> {
        self.add_server(server).await?;
        Ok(self)
    }

    pub async fn with_servers<'a>(
        mut self,
        servers: impl IntoIterator<Item = &'a ServerConfig>,
        timeout_secs: u64,
    ) -> Result<Self> {
        self.add_servers(servers, timeout_secs).await?;
        Ok(self)
    }

    pub fn with_callback(mut self, callback: &CallbackConfig) -> Result<Self> {
        self.add_callback(callback)?;
        Ok(self)
    }

    pub fn with_callbacks<'a>(
        mut self,
        callbacks: impl IntoIterator<Item = &'a CallbackConfig>,
    ) -> Result<Self> {
        self.add_callbacks(callbacks)?;
        Ok(self)
    }

    // --------------- Registrations functions ---------------

    pub async fn add_server(&mut self, server: &ServerConfig) -> Result<()> {
        self.add_servers([server], 30).await?;
        Ok(())
    }

    pub async fn add_servers<'a>(
        &mut self,
        servers: impl IntoIterator<Item = &'a ServerConfig>,
        timeout_secs: u64,
    ) -> Result<()> {
        let timeout = Duration::from_secs(timeout_secs);
        let mut tasks = vec![];
        let mut servers_to_add = vec![];
        for server in servers {
            servers_to_add.push(server.clone());
            let server = server.clone();
            let task = tokio::spawn(async move {
                let result = tokio::time::timeout(timeout, Self::server_to_toolset(&server)).await;

                match result {
                    Ok(Ok(tool_set)) => Ok(tool_set),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(Error::Message(format!(
                        "Registration timed out after {}s for MCP server {} ({})",
                        timeout.as_secs(),
                        &server.name,
                        server.display_target()
                    ))),
                }
            });

            tasks.push(task);
        }

        // join and unpack results
        let results = futures::future::join_all(tasks).await;
        let mut tool_sets = vec![];
        for result in results {
            tool_sets.push(result.map_err(|e| {
                Error::Message(format!("Failed joining parallel MCP registration: {e:?}"))
            })??);
        }

        // check for ToolSet conflicts & add to self
        for tool_set in tool_sets {
            self.add_tool_set(tool_set)?;
        }

        // add server configs
        self.servers.extend(servers_to_add);

        Ok(())
    }

    async fn server_to_toolset(server: &ServerConfig) -> Result<ToolSet> {
        // Connect to the MCP server (this is the slow operation)
        debug!(
            "Connecting to MCP server '{}'({})...",
            &server.name,
            server.display_target()
        );
        let mcp_client = server.connect().await?;

        debug!(
            "Successfully connected to '{}', listing tools...",
            server.name
        );

        // List all tools (another potentially slow operation)
        let listed_tools = mcp_client.list_all_tools().await?;
        debug!("Found {} tools from '{}'", listed_tools.len(), server.name);

        // Convert MCP tools to pctx tools
        let mut tools = vec![];
        for mcp_tool in listed_tools {
            let input_schema =
                serde_json::from_value::<pctx_codegen::RootSchema>(json!(mcp_tool.input_schema))
                    .map_err(|e| {
                        Error::Message(format!(
                            "Failed parsing inputSchema as json schema for tool `{}`: {e}",
                            &mcp_tool.name
                        ))
                    })?;
            let output_schema = if let Some(o) = &mcp_tool.output_schema {
                Some(
                    serde_json::from_value::<pctx_codegen::RootSchema>(json!(o)).map_err(|e| {
                        Error::Message(format!(
                            "Failed parsing outputSchema as json schema for tool `{}`: {e}",
                            &mcp_tool.name
                        ))
                    })?,
                )
            } else {
                None
            };

            tools.push(
                Tool::new_mcp(
                    &mcp_tool.name,
                    mcp_tool.description.map(String::from),
                    input_schema,
                    output_schema,
                )
                .map_err(|e| {
                    Error::Message(format!("Failed to create tool `{}`: {e}", &mcp_tool.name))
                })?,
            );
        }

        let description = mcp_client
            .peer_info()
            .and_then(|p| p.server_info.title.clone())
            .unwrap_or(format!("MCP server at {}", server.display_target()));

        let tool_set = ToolSet::new(&server.name, &description, tools);

        info!(
            "Successfully initialized MCP server '{}' with {} tools",
            server.name,
            tool_set.tools.len()
        );

        Ok(tool_set)
    }

    pub fn add_callbacks<'a>(
        &mut self,
        callbacks: impl IntoIterator<Item = &'a CallbackConfig>,
    ) -> Result<()> {
        for callback in callbacks {
            self.add_callback(callback)?;
        }
        Ok(())
    }

    // Generates a Tool and add it to the correct Toolset from the given callback config
    pub fn add_callback(&mut self, callback: &CallbackConfig) -> Result<()> {
        debug!(callback =? callback.id(), "Adding callback tool {}", callback.id());

        // find the correct toolset & check for clashes
        let idx = self
            .tool_sets
            .iter()
            .position(|s| s.name == callback.namespace)
            .unwrap_or_else(|| {
                let idx = self.tool_sets.len();
                self.tool_sets
                    .push(ToolSet::new(&callback.namespace, "", vec![]));
                idx
            });
        let tool_set = &mut self.tool_sets[idx];

        if tool_set.tools.iter().any(|t| t.name == callback.name) {
            return Err(Error::Message(format!(
                "ToolSet `{}` already has a tool with name `{}`. Tool names must be unique within tool sets",
                &tool_set.name, &callback.name
            )));
        }

        // convert callback config into tool
        let input_schema = if let Some(i) = &callback.input_schema {
            serde_json::from_value::<pctx_codegen::RootSchema>(json!(i)).map_err(|e| {
                Error::Message(format!(
                    "Failed parsing inputSchema as json schema for tool `{}`: {e}",
                    &callback.name
                ))
            })?
        } else {
            // TODO: better empty input schema support
            serde_json::from_value::<pctx_codegen::RootSchema>(json!({})).unwrap()
        };
        let output_schema = if let Some(o) = &callback.output_schema {
            Some(
                serde_json::from_value::<pctx_codegen::RootSchema>(json!(o)).map_err(|e| {
                    Error::Message(format!(
                        "Failed parsing outputSchema as json schema for tool `{}`: {e}",
                        &callback.name
                    ))
                })?,
            )
        } else {
            None
        };
        let tool = Tool::new_callback(
            &callback.name,
            callback.description.clone(),
            input_schema,
            output_schema,
        )?;

        // add tool & it's configuration
        tool_set.tools.push(tool);
        self.callbacks.push(callback.clone());

        Ok(())
    }

    pub fn add_tool_set(&mut self, tool_set: ToolSet) -> Result<()> {
        if self.tool_sets.iter().any(|t| t.name == tool_set.name) {
            return Err(Error::Message(format!(
                "CodeMode already has ToolSet with name: {}",
                tool_set.name
            )));
        }

        self.tool_sets.push(tool_set);

        Ok(())
    }

    // --------------- Accessor functions ---------------

    /// Returns an immutable reference to the registered ToolSets
    pub fn tool_sets(&self) -> &[pctx_codegen::ToolSet] {
        &self.tool_sets
    }

    /// Returns an immutable reference to the registered server configurations
    pub fn servers(&self) -> &[ServerConfig] {
        &self.servers
    }

    /// Returns an immutable reference to the registered callback configurations
    pub fn callbacks(&self) -> &[CallbackConfig] {
        &self.callbacks
    }

    pub fn allowed_hosts(&self) -> HashSet<String> {
        self.servers
            .iter()
            .filter_map(|s| {
                let http_cfg = s.http()?;
                let host = http_cfg.url.host()?;
                let allowed = if let Some(port) = http_cfg.url.port() {
                    format!("{host}:{port}")
                } else {
                    let default_port = if http_cfg.url.scheme() == "https" {
                        443
                    } else {
                        80
                    };
                    format!("{host}:{default_port}")
                };
                Some(allowed)
            })
            .collect()
    }

    // --------------- Code-Mode Tools ---------------

    /// Returns internal tool sets as minimal code interfaces
    pub fn list_functions(&self) -> ListFunctionsOutput {
        let mut namespaces = vec![];
        let mut functions = vec![];

        for tool_set in &self.tool_sets {
            if tool_set.tools.is_empty() {
                // skip sets with no tools
                continue;
            }

            namespaces.push(tool_set.namespace_interface(false));

            functions.extend(tool_set.tools.iter().map(|t| ListedFunction {
                namespace: tool_set.namespace.clone(),
                name: t.fn_name.clone(),
                description: t.description.clone(),
            }));
        }

        ListFunctionsOutput {
            code: pctx_codegen::format::format_d_ts(&namespaces.join("\n\n")),
            functions,
        }
    }

    /// Gets the full typed interface for the requested functions
    pub fn get_function_details(&self, input: GetFunctionDetailsInput) -> GetFunctionDetailsOutput {
        // sort by mod
        let mut by_mod: HashMap<String, HashSet<String>> = HashMap::default();
        for fn_id in &input.functions {
            by_mod
                .entry(fn_id.mod_name.clone())
                .or_default()
                .insert(fn_id.fn_name.clone());
        }

        let mut namespaces = vec![];
        let mut functions = vec![];

        for tool_set in &self.tool_sets {
            if let Some(fn_names) = by_mod.get(&tool_set.namespace) {
                // filter tools based on requested fn names
                let tools: Vec<&pctx_codegen::Tool> = tool_set
                    .tools
                    .iter()
                    .filter(|t| fn_names.contains(&t.fn_name))
                    .collect();

                if !tools.is_empty() {
                    // code definition
                    let fn_details: Vec<String> =
                        tools.iter().map(|t| t.fn_signature(true)).collect();
                    namespaces.push(tool_set.wrap_with_namespace(&fn_details.join("\n\n")));

                    // struct output
                    functions.extend(tools.iter().map(|t| FunctionDetails {
                        listed: ListedFunction {
                            namespace: tool_set.namespace.clone(),
                            name: t.fn_name.clone(),
                            description: t.description.clone(),
                        },
                        input_type: t.input_signature.clone(),
                        output_type: t.output_signature.clone(),
                        types: t.types.clone(),
                    }));
                }
            }
        }

        let code = if namespaces.is_empty() {
            "// No namespaces/functions match the request".to_string()
        } else {
            pctx_codegen::format::format_d_ts(&namespaces.join("\n\n"))
        };

        GetFunctionDetailsOutput { code, functions }
    }

    #[instrument(skip(self, callback_registry), ret(Display), err)]
    pub async fn execute(
        &self,
        code: &str,
        callback_registry: Option<CallbackRegistry>,
    ) -> Result<ExecuteOutput> {
        let registry = callback_registry.unwrap_or_default();
        // Format for logging only
        let formatted_code = pctx_codegen::format::format_ts(code);

        debug!(
            code_from_llm = %code,
            formatted_code = %formatted_code,
            code_length = code.len(),
            callbacks =? registry.ids(),
            "Received code to execute"
        );

        // confirm all configured callbacks in the CodeMode interface have
        // registered callback functions
        let missing_ids: Vec<String> = self
            .callbacks
            .iter()
            .filter_map(|c| {
                if registry.has(&c.id()) {
                    None
                } else {
                    Some(c.id())
                }
            })
            .collect();
        if !missing_ids.is_empty() {
            return Err(Error::Message(format!(
                "Missing configured callbacks in registry with ids: {missing_ids:?}"
            )));
        }

        // generate the full script to be executed
        let namespaces: Vec<String> = self
            .tool_sets
            .iter()
            .filter_map(|s| {
                if s.tools.is_empty() {
                    None
                } else {
                    Some(s.namespace())
                }
            })
            .collect();

        // Put LLM code at the top, then namespaces below
        let to_execute = format!(
            "{code}\n\n{namespaces}\n\nexport default await run();\n",
            namespaces = namespaces.join("\n\n"),
        );

        debug!(to_execute = %to_execute, "Executing code in sandbox");

        let options = pctx_executor::ExecuteOptions::new()
            .with_allowed_hosts(self.allowed_hosts().into_iter().collect())
            .with_servers(self.servers.clone())
            .with_callbacks(registry);

        let execution_res = pctx_executor::execute(&to_execute, options).await?;

        if execution_res.success {
            debug!("Sandbox execution completed successfully");
        } else {
            warn!("Sandbox execution failed: {:?}", execution_res.stderr);
        }

        Ok(ExecuteOutput {
            success: execution_res.success,
            stdout: execution_res.stdout,
            stderr: execution_res.stderr,
            output: execution_res.output,
        })
    }
}
