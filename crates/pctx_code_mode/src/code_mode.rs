use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use pctx_code_execution_runtime::CallbackRegistry;
use pctx_codegen::{Tool, ToolSet};
use pctx_config::server::ServerConfig;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, instrument, warn};
use uuid::Uuid;

use crate::{
    Error, Result,
    model::{
        CallbackConfig, ExecuteOutput, FunctionDetails, GetFunctionDetailsInput,
        GetFunctionDetailsOutput, ListFunctionsOutput, ListedFunction,
    },
    search::ToolSearchIndex,
};
pub use crate::search::SearchResult;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct CodeMode {
    // Codegen interfaces
    pub tool_sets: Vec<pctx_codegen::ToolSet>,

    // configurations
    pub servers: Vec<ServerConfig>,
    pub callbacks: Vec<CallbackConfig>,

    // Runtime-only: BM25 search index (lazy-initialized, not serialized)
    #[serde(skip)]
    search_index: OnceLock<ToolSearchIndex>,
}

impl Clone for CodeMode {
    fn clone(&self) -> Self {
        Self {
            tool_sets: self.tool_sets.clone(),
            servers: self.servers.clone(),
            callbacks: self.callbacks.clone(),
            // Fresh index for cloned instance (will be rebuilt on first search)
            search_index: OnceLock::new(),
        }
    }
}

impl CodeMode {
    /// Search for functions matching the query using BM25 ranking
    ///
    /// Returns Tool IDs with relevance scores > 0, ordered by relevance.
    /// Use `get_tool_by_id` to look up the full tool details.
    pub fn search_functions(&self, query: &str, k: usize) -> Vec<SearchResult> {
        let index = self
            .search_index
            .get_or_init(|| ToolSearchIndex::from_tool_sets(&self.tool_sets));

        index.search(query, k)
    }

    /// Get a tool by its unique ID
    pub fn get_tool_by_id(&self, id: Uuid) -> Option<(&Tool, &str)> {
        for tool_set in &self.tool_sets {
            if let Some(tool) = tool_set.tools.iter().find(|t| t.id == id) {
                return Some((tool, &tool_set.namespace));
            }
        }
        None
    }

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

    // Generates a Tool and add it to the correct Toolset from the given callback config
    pub fn add_callback(&mut self, cfg: &CallbackConfig) -> Result<()> {
        // find the correct toolset & check for clashes
        let tool_set =
            if let Some(exists) = self.tool_sets.iter_mut().find(|s| s.name == cfg.namespace) {
                exists
            } else {
                self.tool_sets
                    .push(ToolSet::new(&cfg.namespace, "", vec![]));
                self.tool_sets
                    .iter_mut()
                    .find(|s| s.name == cfg.namespace)
                    .unwrap()
            };

        if tool_set.tools.iter().any(|t| t.name == cfg.name) {
            return Err(Error::Message(format!(
                "ToolSet `{}` already has a tool with name `{}`. Tool names must be unique within tool sets",
                &tool_set.name, &cfg.name
            )));
        }

        // convert callback config into tool
        let input_schema = if let Some(i) = &cfg.input_schema {
            Some(
                serde_json::from_value::<pctx_codegen::RootSchema>(json!(i)).map_err(|e| {
                    Error::Message(format!(
                        "Failed parsing inputSchema as json schema for tool `{}`: {e}",
                        &cfg.name
                    ))
                })?,
            )
        } else {
            None
        };
        let output_schema = if let Some(o) = &cfg.output_schema {
            Some(
                serde_json::from_value::<pctx_codegen::RootSchema>(json!(o)).map_err(|e| {
                    Error::Message(format!(
                        "Failed parsing outputSchema as json schema for tool `{}`: {e}",
                        &cfg.name
                    ))
                })?,
            )
        } else {
            None
        };
        let tool = Tool::new_callback(
            &cfg.name,
            cfg.description.clone(),
            input_schema.unwrap(), // TODO: optional input schemas
            output_schema,
        )?;

        // add tool & it's configuration
        tool_set.tools.push(tool);
        self.callbacks.push(cfg.clone());

        Ok(())
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
}
