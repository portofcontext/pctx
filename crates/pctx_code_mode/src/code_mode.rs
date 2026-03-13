use pctx_codegen::{Tool, ToolSet};
use pctx_config::{ToolDisclosure, server::ServerConfig};
use pctx_registry::PctxRegistry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
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

    // Virtual filesystem for just-bash exploration
    virtual_fs: HashMap<String, String>,
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
        for server in servers {
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
        let joined_results = futures::future::join_all(tasks).await;
        let mut results = vec![];
        for result in joined_results {
            results.push(result.map_err(|e| {
                Error::Message(format!("Failed joining parallel MCP registration: {e:?}"))
            })??);
        }

        // check for ToolSet conflicts & add to self
        for (tool_set, server_cfg) in results {
            self.add_tool_set(tool_set)?;
            self.servers.push(server_cfg)
        }

        Ok(())
    }

    async fn server_to_toolset(server: &ServerConfig) -> Result<(ToolSet, ServerConfig)> {
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
        for mcp_tool in &listed_tools {
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
                Tool::new(
                    &mcp_tool.name,
                    mcp_tool.description.clone().map(String::from),
                    Some(input_schema),
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

        let tool_set = ToolSet::new(Some(server.name.clone()), &description, tools);

        info!(
            "Successfully initialized MCP server '{}' with {} tools",
            server.name,
            tool_set.tools.len()
        );

        Ok((tool_set, server.clone()))
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
                    .push(ToolSet::new(callback.namespace.clone(), "", vec![]));
                idx
            });
        let tool_set = &mut self.tool_sets[idx];

        if tool_set.tools.iter().any(|t| t.name == callback.name) {
            return Err(Error::Message(format!(
                "ToolSet `{}` already has a tool with name `{}`. Tool names must be unique within tool sets",
                tool_set.name.as_deref().unwrap_or_default(),
                &callback.name
            )));
        }

        // convert callback config into tool
        let input_schema = serde_json::from_value::<Option<pctx_codegen::RootSchema>>(json!(
            &callback.input_schema
        ))
        .map_err(|e| {
            Error::Message(format!(
                "Failed parsing inputSchema as json schema for tool `{}`: {e}",
                &callback.name
            ))
        })?;
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
        let tool = Tool::new(
            &callback.name,
            callback.description.clone(),
            input_schema,
            output_schema,
        )?;

        // add tool & it's configuration
        tool_set.tools.push(tool);
        self.callbacks.push(callback.clone());
        self.refresh_virtual_fs();

        Ok(())
    }

    pub fn add_tool_set(&mut self, tool_set: ToolSet) -> Result<()> {
        if self.tool_sets.iter().any(|t| t.name == tool_set.name) {
            return Err(Error::Message(format!(
                "CodeMode already has ToolSet with name: {}",
                tool_set.name.unwrap_or_default()
            )));
        }

        self.tool_sets.push(tool_set);
        self.refresh_virtual_fs();

        Ok(())
    }

    /// Regenerates the virtual filesystem from current tool_sets
    fn refresh_virtual_fs(&mut self) {
        self.virtual_fs = self
            .load_tools_into_bashable_filesystem()
            .unwrap_or_default();
    }

    /// Initializes code mode interface into the filesystem
    /// Returns a map of virtual file paths to their contents for use with just-bash
    pub fn load_tools_into_bashable_filesystem(&self) -> Result<HashMap<String, String>> {
        let mut files = HashMap::new();

        // Token-efficient README. Shell-doc style: one .d.ts path per line under each
        // namespace section. Each .d.ts contains: InputType, OutputType, async fn signature.
        let mut readme = String::from(
            "# TypeScript SDK\n\
             Usage: `Namespace.fn({ params })` — each .d.ts has the InputType, OutputType, and fn signature.\n\
             `cat <path>.d.ts` before calling — param names cannot be inferred.\n\n",
        );

        for tool_set in &self.tool_sets {
            if tool_set.tools.is_empty() {
                continue;
            }

            // e.g. "## Tools"
            readme.push_str(&format!("## {}\n", tool_set.pascal_namespace()));

            // One line per function: "Namespace/fn.d.ts  # description"
            // `cat` omitted since it's stated once in the header above.
            let func_list: Vec<String> = tool_set
                .tools
                .iter()
                .map(|tool| {
                    let desc = tool
                        .description
                        .as_deref()
                        .unwrap_or("")
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_lowercase();

                    // Create file for this function under /sdk/
                    let tool_file_path =
                        format!("/sdk/{}/{}.d.ts", tool_set.pascal_namespace(), tool.fn_name);
                    let tool_code = tool.ts_fn_signature(true);
                    let formatted = pctx_codegen::format::format_d_ts(&tool_code);
                    files.insert(tool_file_path, formatted);

                    if desc.is_empty() {
                        format!("{}/{}.d.ts", tool_set.pascal_namespace(), tool.fn_name)
                    } else {
                        format!(
                            "{}/{}.d.ts  # {}",
                            tool_set.pascal_namespace(),
                            tool.fn_name,
                            desc
                        )
                    }
                })
                .collect();

            readme.push_str(&format!("{}\n\n", func_list.join("\n")));
        }

        files.insert("/sdk/README.md".to_string(), readme);

        Ok(files)
    }

    // --------------- Accessor functions ---------------

    /// Returns an immutable reference to the registered ToolSets
    pub fn tool_sets(&self) -> &[pctx_codegen::ToolSet] {
        &self.tool_sets
    }

    /// Returns an immutable reference to the registered ToolSets
    /// representing upstream servers
    pub fn server_tool_sets(&self) -> Vec<(&ServerConfig, &pctx_codegen::ToolSet)> {
        // let server_names: Vec<&str> = self.servers.iter().map(|s| s.0.name.as_str()).collect();
        self.tool_sets
            .iter()
            .filter_map(|ts| {
                if let Some(server_cfg) = self
                    .servers
                    .iter()
                    .find(|s| Some(s.name.as_str()) == ts.name.as_deref())
                {
                    Some((server_cfg, ts))
                } else {
                    None
                }
                // ts.name
                //     .as_deref()
                //     .is_some_and(|n| server_names.contains(&n))
            })
            .collect()
    }

    /// Returns an immutable reference to the registered server configurations
    pub fn servers(&self) -> &[ServerConfig] {
        &self.servers
    }

    /// Returns an immutable reference to the registered callback configurations
    pub fn callbacks(&self) -> &[CallbackConfig] {
        &self.callbacks
    }

    /// Returns an immutable reference to the virtual filesystem
    /// This contains .d.ts files, README.md, and index.d.ts for just-bash exploration
    pub fn virtual_fs(&self) -> &HashMap<String, String> {
        &self.virtual_fs
    }

    // --------------- Utilities ---------------
    pub fn add_mcp_servers_to_registry(&self, registry: &mut PctxRegistry) -> Result<()> {
        for (cfg, tool_set) in self.server_tool_sets() {
            let tool_names: Vec<String> = tool_set.tools.iter().map(|t| t.name.clone()).collect();
            registry.add_mcp(&tool_names, cfg.clone())?;
        }

        Ok(())
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

            namespaces.push(tool_set.ts_namespace_declaration(false));

            functions.extend(tool_set.tools.iter().map(|t| ListedFunction {
                namespace: tool_set.pascal_namespace(),
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
            if let Some(fn_names) = by_mod.get(&tool_set.pascal_namespace()) {
                // filter tools based on requested fn names
                let tools: Vec<&pctx_codegen::Tool> = tool_set
                    .tools
                    .iter()
                    .filter(|t| fn_names.contains(&t.fn_name))
                    .collect();

                if !tools.is_empty() {
                    // code definition
                    let fn_details: Vec<String> =
                        tools.iter().map(|t| t.ts_fn_signature(true)).collect();
                    namespaces.push(tool_set.ts_wrap_with_namespace(&fn_details.join("\n\n")));

                    // struct output
                    functions.extend(tools.iter().map(|t| FunctionDetails {
                        listed: ListedFunction {
                            namespace: tool_set.pascal_namespace(),
                            name: t.fn_name.clone(),
                            description: t.description.clone(),
                        },
                        input_type: t.input_signature().unwrap_or_default(),
                        output_type: t.output_signature(),
                        types: t.types(),
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

    /// Execute bash commands directly in the virtual filesystem
    #[instrument(skip(self), ret(Display), err)]
    pub async fn execute_bash(&self, command: &str) -> Result<ExecuteOutput> {
        debug!(command = %command, "Executing bash command");

        // Wrap bash command in async IIFE and export the result
        // The result from bashFs.exec() contains: { stdout: string, stderr: string, exitCode: number }
        let to_execute = format!(
            r#"// Initialize bash filesystem with tool definitions
const bashFs = new justBash({{
    files: {virtual_fs_json},
    cwd: "/sdk",
}});

// Execute the bash command in an async IIFE
const result = await (async () => {{
    return await bashFs.exec({command});
}})();

export default result;"#,
            virtual_fs_json = json!(self.virtual_fs),
            command = json!(command),
        );

        debug!(to_execute = %to_execute, "Executing bash in sandbox");

        let execution_res =
            pctx_executor::execute(&to_execute, pctx_executor::ExecuteOptions::new()).await?;

        // Extract stdout and stderr from the bash result object
        // The output field contains the result object: { stdout, stderr, exitCode }
        let (bash_stdout, bash_stderr, exit_code) = if execution_res.success {
            if let Some(output_value) = &execution_res.output {
                if let Some(result_obj) = output_value.as_object() {
                    let stdout = result_obj
                        .get("stdout")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let stderr = result_obj
                        .get("stderr")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let exit_code = result_obj
                        .get("exitCode")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    (stdout, stderr, exit_code)
                } else {
                    (String::new(), String::new(), 0)
                }
            } else {
                (String::new(), String::new(), 0)
            }
        } else {
            // If execution failed at the TypeScript level, use the execution error
            (String::new(), execution_res.stderr.clone(), 1)
        };

        let success = execution_res.success && exit_code == 0;

        if success {
            debug!("Bash execution completed successfully");
        } else {
            warn!(
                "Bash execution failed with exit code {}: {}",
                exit_code, bash_stderr
            );
        }

        Ok(ExecuteOutput {
            success,
            stdout: bash_stdout,
            stderr: bash_stderr,
            output: None,
        })
    }

    /// Execute TypeScript code with access to registered tools and virtual filesystem
    #[instrument(skip(self, registry), ret(Display), err)]
    pub async fn execute_typescript(
        &self,
        code: &str,
        disclosure: ToolDisclosure,
        registry: Option<PctxRegistry>,
    ) -> Result<ExecuteOutput> {
        let mut registry = registry.unwrap_or_default();
        self.add_mcp_servers_to_registry(&mut registry)?;

        // Format for logging only
        let formatted_code = pctx_codegen::format::format_ts(code);

        debug!(
            code_from_llm = %code,
            formatted_code = %formatted_code,
            code_length = code.len(),
            callbacks =? registry.ids(),
            disclosure =? disclosure,
            "Received TypeScript code to execute"
        );

        // confirm all configured tool IDs the CodeMode interface have
        // registered actions
        let all_ids: Vec<String> = self.tool_sets.iter().flat_map(|ts| ts.tool_ids()).collect();
        let missing_ids: Vec<String> = all_ids
            .iter()
            .filter_map(|c| {
                if registry.has(c) {
                    None
                } else {
                    Some(c.clone())
                }
            })
            .collect();
        if !missing_ids.is_empty() {
            return Err(Error::Message(format!(
                "Registry missing ids: {missing_ids:?}"
            )));
        }

        let to_execute = match disclosure {
            ToolDisclosure::Catalog | ToolDisclosure::Filesystem => {
                // generate the full script to be executed
                let namespaces: Vec<String> = self
                    .tool_sets
                    .iter()
                    .filter_map(|s| {
                        if s.tools.is_empty() {
                            None
                        } else {
                            Some(s.ts_namespace_impl())
                        }
                    })
                    .collect();

                format!(
                    "{code}\n\n{namespaces}\n\nexport default await run();",
                    namespaces = pctx_codegen::format::format_ts(&namespaces.join("\n\n")),
                )
            }
            ToolDisclosure::Sidecar => {
                let invoke_map_entries: Vec<String> = self
                    .tool_sets
                    .iter()
                    .flat_map(|ts| {
                        ts.tools
                            .iter()
                            .map(|t| t.ts_invoke_map_entry(ts.name.as_deref()))
                    })
                    .collect();
                let types: Vec<String> = self
                    .tool_sets
                    .iter()
                    .flat_map(|ts| {
                        ts.tools.iter().filter_map(|t| {
                            let types = t.types();
                            if types.is_empty() { None } else { Some(types) }
                        })
                    })
                    .collect();

                let invoke_interface = format!(
                    r#"
                        type InvokeMap = {{
                        {invoke_map_entries}
                        }};

                        type InvokeCall<K extends keyof InvokeMap> = 
                        undefined extends InvokeMap[K]["args"]
                            ? {{ name: K; arguments?: InvokeMap[K]["args"] }}
                            : {{ name: K; arguments: InvokeMap[K]["args"] }};

                        async function invoke<K extends keyof InvokeMap>(call: InvokeCall<K>): Promise<InvokeMap[K]["returns"]> {{
                            return await invokeInternal(call);
                        }}

                        {types}
                    "#,
                    invoke_map_entries = invoke_map_entries.join("\n  "),
                    types = types.join("\n\n")
                );
                format!(
                    "{code}\n\n{invoke_interface}\n\nexport default await run();",
                    invoke_interface = pctx_codegen::format::format_ts(&invoke_interface)
                )
            }
        };

        debug!(to_execute = %to_execute, "Executing TypeScript in sandbox");

        let execution_res = pctx_executor::execute(
            &to_execute,
            pctx_executor::ExecuteOptions::new().with_registry(registry),
        )
        .await?;

        if execution_res.success {
            debug!("TypeScript execution completed successfully");
        } else {
            warn!("TypeScript execution failed: {:?}", execution_res.stderr);
        }

        Ok(ExecuteOutput {
            success: execution_res.success,
            stdout: execution_res.stdout,
            stderr: execution_res.stderr,
            output: execution_res.output,
        })
    }
}
