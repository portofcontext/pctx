use anyhow::Result;
use codegen::generate_docstring;
use indexmap::{IndexMap, IndexSet};
use log::info;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};
use serde_json::json;

use crate::mcp::upstream::UpstreamMcp;

type McpResult<T> = Result<T, McpError>;

#[derive(Clone)]
pub(crate) struct PtcxTools {
    allowed_hosts: Vec<String>,
    upstream: Vec<UpstreamMcp>,
    tool_router: ToolRouter<PtcxTools>,
}
#[tool_router]
impl PtcxTools {
    pub(crate) fn new(allowed_hosts: Vec<String>) -> Self {
        Self {
            allowed_hosts,
            upstream: vec![],
            tool_router: Self::tool_router(),
        }
    }

    pub(crate) fn with_upstream_mcps(mut self, upstream: Vec<UpstreamMcp>) -> Self {
        self.upstream = upstream;
        self
    }

    #[tool(
        title = "List Functions",
        description = "ALWAYS USE THIS TOOL FIRST to list all available functions organized by namespace.

        WORKFLOW:
        1. Start here - Call this tool to see what functions are available
        2. Then call get_function_details() for specific functions you need to understand
        3. Finally call execute() to run your TypeScript code

        This returns function signatures without full details."
    )]
    async fn list_functions(&self) -> McpResult<CallToolResult> {
        let namespaces: Vec<String> = self
            .upstream
            .iter()
            .map(|m| {
                let fns: Vec<String> = m.tools.iter().map(|(_, t)| t.fn_signature(false)).collect();

                format!(
                    "{docstring}
namespace {namespace} {{
  {fns}
}}",
                    docstring = generate_docstring(&m.description),
                    namespace = &m.namespace,
                    fns = fns.join("\n\n")
                )
            })
            .collect();

        let namespaced_functions = codegen::format::format_d_ts(&namespaces.join("\n\n"));

        Ok(CallToolResult::success(vec![Content::text(
            namespaced_functions,
        )]))
    }

    #[tool(
        title = "Get Function Details",
        description = "Get detailed information about specific functions you want to use.

        WHEN TO USE: After calling list_functions(), use this to learn about parameter types, return values, and usage for specific functions.

        REQUIRED FORMAT: Functions must be specified as 'namespace.functionName' (e.g., 'Namespace.apiPostSearch')

        This tool is lightweight and only returns details for the functions you request, avoiding unnecessary token usage.
        Only request details for functions you actually plan to use in your code.

        NOTE ON RETURN TYPES:
        - If a function returns Promise<any>, the MCP server didn't provide an output schema
        - The actual value is a parsed object (not a string) - access properties directly
        - Don't use JSON.parse() on the results - they're already JavaScript objects"
    )]
    async fn get_function_details(
        &self,
        Parameters(GetFunctionDetailsInput { functions }): Parameters<GetFunctionDetailsInput>,
    ) -> McpResult<CallToolResult> {
        // organize tool input by namespace and handle any deduping
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
                        "{docstring}
namespace {namespace} {{
  {fns}
}}",
                        docstring = generate_docstring(&mcp.description),
                        namespace = &mcp.namespace,
                        fns = fn_details.join("\n\n")
                    ));
                }
            }
        }

        let content = if namespace_details.is_empty() {
            "No namespaces/functions match the request".to_string()
        } else {
            codegen::format::format_d_ts(&namespace_details.join("\n\n"))
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        title = "Execute Code",
        description = "Execute TypeScript code that calls namespaced functions. USE THIS LAST after list_functions() and get_function_details().

        TOKEN USAGE WARNING: This tool could return LARGE responses if your code returns big objects.
        To minimize tokens:
        - Filter/map/reduce data IN YOUR CODE before returning
        - Only return specific fields you need (e.g., return {id: result.id, count: items.length})
        - Use console.log() for intermediate results instead of returning everything
        - Avoid returning full API responses - extract just what you need

        REQUIRED CODE STRUCTURE:
        async function run() {
            // Your code here
            // Call namespace.functionName() - MUST include namespace prefix
            // Process data here to minimize return size
            return onlyWhatYouNeed; // Keep this small!
        }

        IMPORTANT RULES:
        - Functions MUST be called as 'Namespace.functionName' (e.g., 'Notion.apiPostSearch')
        - Only functions from list_functions() are available - no fetch(), fs, or other Node/Deno APIs
        - Variables don't persist between execute() calls - return or log anything you need later
        - Add console.log() statements between API calls to track progress if errors occur
        - Code runs in an isolated Deno sandbox with restricted network access

        RETURN TYPE NOTE:
        - Functions without output schemas show Promise<any> as return type
        - The actual runtime value is already a parsed JavaScript object, NOT a JSON string
        - Do NOT call JSON.parse() on results - they're already objects
        - Access properties directly (e.g., result.data) or inspect with console.log() first
        - If you see 'Promise<any>', the structure is unknown - log it to see what's returned
        "
    )]
    async fn execute(
        &self,
        Parameters(ExecuteInput { code }): Parameters<ExecuteInput>,
    ) -> McpResult<CallToolResult> {
        let registrations = self
            .upstream
            .iter()
            .map(|m| format!("registerMCP({});", &m.registration))
            .collect::<Vec<String>>()
            .join("\n\n");
        let namespaces = self
            .upstream
            .iter()
            .map(|m| {
                let fns: Vec<String> = m.tools.iter().map(|(_, t)| t.fn_impl(&m.name)).collect();

                format!(
                    "{docstring}
namespace {namespace} {{
  {fns}
}}",
                    docstring = generate_docstring(&m.description),
                    namespace = &m.namespace,
                    fns = fns.join("\n\n")
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

        info!("Executing code in sandbox");

        let allowed_hosts = self.allowed_hosts.clone();
        let code_to_execute = to_execute.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<_, anyhow::Error> {
            // Create a new current-thread runtime for Deno ops that use deno_unsync
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to create runtime: {e}"))?;

            rt.block_on(async {
                deno_executor::execute(&code_to_execute, Some(allowed_hosts))
                    .await
                    .map_err(|e| anyhow::anyhow!("Execution error: {e}"))
            })
        })
        .await
        .map_err(|e| {
            log::error!("Task join failed: {e}");
            McpError::internal_error(format!("Task join failed: {e}"), None)
        })?
        .map_err(|e| {
            log::error!("Sandbox execution error: {e}");
            McpError::internal_error(format!("Execution failed: {e}"), None)
        })?;

        if result.success {
            log::info!("Sandbox execution completed successfully");
        } else {
            log::warn!("Sandbox execution failed: {:?}", result.stderr);
        }

        let text_result = format!(
            "Code Executed Successfully: {success}

# Return Value
```json
{return_val}
```

# STDOUT
{stdout}

# STDERR
{stderr}
",
            success = result.success,
            return_val = serde_json::to_string_pretty(&result.output)
                .unwrap_or(json!(result.output).to_string()),
            stdout = result.stdout,
            stderr = result.stderr,
        );

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(text_result)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(text_result)]))
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct GetFunctionDetailsInput {
    /// List of functions to get details of. Functions should be in the form "<namespace>.<function name>".
    /// e.g. If there is a function `getData` within the `DataApi` namespace the value provided in this field is "DataApi.getData"
    pub functions: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct ExecuteInput {
    /// Typescript code to execute.
    /// Example:
    /// async function ``run()`` {
    ///   // YOUR CODE GOES HERE e.g. const result = await ``client.method();``
    ///   // ALWAYS RETURN THE RESULT e.g. return result;
    /// }
    ///
    pub code: String,
}

#[tool_handler]
impl ServerHandler for PtcxTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(format!(
                "This server provides tools to explore SDK functions and execute SDK scripts for the following services: {}",
                self.upstream
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            )),
        }
    }
}
