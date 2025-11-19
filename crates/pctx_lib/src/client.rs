use anyhow::Result;
use codegen::generate_docstring;
use indexmap::{IndexMap, IndexSet};
use tracing::{debug, warn};

use crate::config::{SdkConfig, ServerConfig};
use crate::upstream::UpstreamMcp;

/// Main client for interacting with upstream MCP servers and executing code
#[derive(Clone)]
pub struct PctxClient {
    config: SdkConfig,
    upstream: Vec<UpstreamMcp>,
}

impl PctxClient {
    /// Create a new PCTX client with the given configuration
    ///
    /// # Arguments
    /// * `config` - SDK configuration including upstream servers and allowed hosts
    pub fn new(config: SdkConfig) -> Self {
        Self {
            config,
            upstream: vec![],
        }
    }

    /// Create a new PCTX client from a config file
    ///
    /// # Arguments
    /// * `path` - Path to the JSON config file
    ///
    /// # Errors
    /// Returns an error if the file doesn't exist or contains invalid JSON
    pub fn from_config(path: &str) -> Result<Self> {
        let config = SdkConfig::from_file(path)?;
        Ok(Self::new(config))
    }

    /// Add an upstream MCP server by connecting and introspecting its tools
    ///
    /// # Arguments
    /// * `server` - Server configuration to connect to
    pub async fn add_server(&mut self, server: &ServerConfig) -> Result<()> {
        let upstream = UpstreamMcp::from_server(server).await?;
        self.upstream.push(upstream);
        Ok(())
    }

    /// Initialize all servers from the config
    pub async fn init_servers(&mut self) -> Result<()> {
        for server in &self.config.servers.clone() {
            self.add_server(server).await?;
        }
        Ok(())
    }

    /// Set upstream MCP servers directly (builder pattern)
    pub fn with_upstream(mut self, upstream: Vec<UpstreamMcp>) -> Self {
        self.upstream = upstream;
        self
    }

    /// List all available functions from upstream MCP servers
    ///
    /// Returns a formatted TypeScript declaration of all namespaced functions
    pub fn list_functions(&self) -> Result<String> {
        let namespaces: Vec<String> = self
            .upstream
            .iter()
            .map(|m| {
                let fns: Vec<String> = m.tools.iter().map(|(_, t)| t.fn_signature(false)).collect();

                format!(
                    "{docstring}\nnamespace {namespace} {{\n  {fns}\n}}",
                    docstring = generate_docstring(&m.description),
                    namespace = &m.namespace,
                    fns = fns.join("\n\n  ")
                )
            })
            .collect();

        let namespaced_functions = codegen::format::format_d_ts(&namespaces.join("\n\n"));
        Ok(namespaced_functions)
    }

    /// Get detailed information about specific functions
    ///
    /// # Arguments
    /// * `functions` - List of functions in format "Namespace.functionName"
    ///
    /// Returns TypeScript declarations with full type definitions
    pub fn get_function_details(&self, functions: Vec<String>) -> Result<String> {
        // Organize tool input by namespace and handle any deduping
        let mut by_namespace: IndexMap<String, IndexSet<String>> = IndexMap::new();
        for func in functions {
            let parts: Vec<&str> = func.split('.').collect();
            if parts.len() != 2 {
                // incorrect format
                continue;
            }
            by_namespace
                .entry(parts[0].to_string())
                .or_default()
                .insert(parts[1].to_string());
        }

        let mut namespace_details = vec![];

        for (namespace, functions) in by_namespace {
            if let Some(mcp) = self.upstream.iter().find(|m| m.namespace == namespace) {
                let mut fn_details = vec![];
                for fn_name in functions {
                    if let Some(tool) = mcp.tools.get(&fn_name) {
                        fn_details.push(tool.fn_signature(true));
                    }
                }

                if !fn_details.is_empty() {
                    namespace_details.push(format!(
                        "{docstring}\nnamespace {namespace} {{\n  {fns}\n}}",
                        docstring = generate_docstring(&mcp.description),
                        namespace = &mcp.namespace,
                        fns = fn_details.join("\n\n  ")
                    ));
                }
            }
        }

        let content = if namespace_details.is_empty() {
            "No namespaces/functions match the request".to_string()
        } else {
            codegen::format::format_d_ts(&namespace_details.join("\n\n"))
        };

        Ok(content)
    }

    /// Execute TypeScript code in a sandbox with access to upstream MCP functions
    ///
    /// # Arguments
    /// * `code` - TypeScript code to execute (should define `async function run()`)
    ///
    /// Returns execution result including output, stdout, stderr, and any errors
    pub async fn execute(&self, code: &str) -> Result<deno_executor::ExecuteResult> {
        debug!(code_length = code.len(), "Executing code in sandbox");

        // Generate MCP registrations
        let registrations = self
            .upstream
            .iter()
            .map(|m| format!("registerMCP({});", &m.registration))
            .collect::<Vec<String>>()
            .join("\n\n");

        // Generate namespace implementations
        let namespaces = self
            .upstream
            .iter()
            .map(|m| {
                let fns: Vec<String> = m.tools.iter().map(|(_, t)| t.fn_impl(&m.name)).collect();

                format!(
                    "{docstring}\nnamespace {namespace} {{\n  {fns}\n}}",
                    docstring = generate_docstring(&m.description),
                    namespace = &m.namespace,
                    fns = fns.join("\n\n  ")
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n");

        let to_execute = format!(
            "
{registrations}

{namespaces}

{code}

export default await run();"
        );

        // Extract allowed hosts from server URLs
        let hosts = self.config.allowed_hosts().ok();
        let code_to_execute = to_execute.clone();

        // Execute in a blocking task since deno_executor uses a current-thread runtime
        let result = tokio::task::spawn_blocking(move || -> Result<_> {
            // Create a new current-thread runtime for Deno ops that use deno_unsync
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;

            rt.block_on(async {
                deno_executor::execute(&code_to_execute, hosts)
                    .await
                    .map_err(|e| anyhow::anyhow!("Execution error: {e}"))
            })
        })
        .await??;

        if result.success {
            debug!("Sandbox execution completed successfully");
        } else {
            warn!("Sandbox execution failed: {:?}", result.stderr);
        }

        Ok(result)
    }

    /// Get the client configuration
    pub fn config(&self) -> &SdkConfig {
        &self.config
    }

    /// Get the list of upstream MCP servers
    pub fn upstream(&self) -> &[UpstreamMcp] {
        &self.upstream
    }

    /// Get allowed hosts for network access (derived from server URLs)
    pub fn allowed_hosts(&self) -> Result<Vec<String>> {
        self.config.allowed_hosts()
    }
}

/// Input structure for get_function_details
#[derive(Debug, serde::Deserialize)]
pub struct GetFunctionDetailsInput {
    /// List of functions to get details of.
    /// Functions should be in the form "<namespace>.<function name>".
    /// e.g. "DataApi.getData"
    pub functions: Vec<String>,
}

/// Input structure for execute
#[derive(Debug, serde::Deserialize)]
pub struct ExecuteInput {
    /// TypeScript code to execute.
    ///
    /// REQUIRED FORMAT:
    /// ```typescript
    /// async function run() {
    ///   // YOUR CODE GOES HERE
    ///   return result;
    /// }
    /// ```
    pub code: String,
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
