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
        description = "List functions organized in namespaces based on available services"
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
        description = "Get details about specific functions as listed in `list_functions`, organized in namespaces"
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
        description = "Runs TypeScript code that can use the namespaced functions listed in `list_functions`
        You are a skilled programmer writing code to interact with the available namespaced functions.

        Always define an async function called `run` that accepts no arguments:

        async function run() {
            // YOUR CODE GOES HERE

            // log results and return output
        }

        The only available methods are returned by the `list_functions` tool, and the inputs and outputs of the methods can be obtained by the `get_function_details` tool, no other inputs or outputs exist.
        When calling the functions you MUST include the namespace. e.g. if e.g. If there is a function `getData` within the `DataApi` namespace, to call the function you must write `DataApi.getData`.
        You will be returned anything that your function returns, plus the results of any console.log statements.
        If any code triggers an error, the tool will return an error response, so you do not need to add error handling unless you want to output something more helpful than the raw error.
        It is not necessary to add comments to code, unless by adding those comments you believe that you can generate better code.
        This code will run in a container, and you will not be able to use the filesystem, `fetch` or otherwise interact with the network calls other than through the namespaced functions you are given.
        Any variables you define won't live between successive uses of this tool, so make sure to return or log any data you might need later.
        Try to avoid logging or returning large objects, try to only return and log the specific fields you may need.
        If you are making calls to multiple methods, add logs between the method calls so in case of a failure, you are aware of how far the execution got.
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
