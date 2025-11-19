use anyhow::Result;
use opentelemetry::KeyValue;
use pctx_config::Config;
use pctx_lib::{PctxClient, SdkConfig, UpstreamMcp};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParam, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParam, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars,
    service::RequestContext,
    tool, tool_router,
};
use serde_json::json;
use tracing::{debug, error, info, instrument, warn};

use crate::utils::metrics::mcp_tool_metrics;

type McpResult<T> = Result<T, McpError>;

/// MCP server wrapper around PctxClient
///
/// This struct handles the MCP server protocol and delegates
/// actual implementation to pctx_lib::PctxClient
#[derive(Clone)]
pub(crate) struct PtcxTools {
    client: PctxClient,
    tool_router: ToolRouter<PtcxTools>,
    cli_config: Config, // Keep CLI config for server info
}

#[tool_router]
impl PtcxTools {
    pub(crate) fn new(config: Config, _allowed_hosts: Vec<String>) -> Self {
        // Convert CLI config to SDK config
        // Note: SDK config derives allowed_hosts from server URLs automatically
        let sdk_config: SdkConfig = config.clone().into();

        let client = PctxClient::new(sdk_config);
        Self {
            client,
            tool_router: Self::tool_router(),
            cli_config: config,
        }
    }

    pub(crate) fn with_upstream_mcps(mut self, upstream: Vec<UpstreamMcp>) -> Self {
        self.client = self.client.with_upstream(upstream);
        self
    }

    fn config(&self) -> &Config {
        &self.cli_config
    }

    fn upstream(&self) -> &[pctx_lib::UpstreamMcp] {
        self.client.upstream()
    }

    #[tool(
        title = "List Functions",
        description = "ALWAYS USE THIS TOOL FIRST to list all available functions organized by namespace.

        WORKFLOW:
        1. Start here - Call this tool to see what functions are available
        2. Then call get_function_details() for specific functions you need to understand
        3. Finally call execute() to run your TypeScript code

        List functions returns function signatures without full details."
    )]
    async fn list_functions(&self) -> McpResult<CallToolResult> {
        let result = self.client.list_functions().map_err(|e| {
            McpError::internal_error(format!("Failed to list functions: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        title = "Get Function Details",
        description = "Get detailed information about specific functions you want to use.

        WHEN TO USE: After calling list_functions(), use this to learn about parameter types, return values, and usage for specific functions.

        REQUIRED FORMAT: Functions must be specified as 'namespace.functionName' (e.g., 'Namespace.apiPostSearch')

        This tool only returns details for the functions you request.
        Only request details for functions you actually plan to use in your code.

        NOTE ON RETURN TYPES:
        - If a function returns Promise<any>, the MCP server didn't provide an output schema
        - Don't use JSON.parse() on the results - they're already JavaScript objects"
    )]
    async fn get_function_details(
        &self,
        Parameters(GetFunctionDetailsInput { functions }): Parameters<GetFunctionDetailsInput>,
    ) -> McpResult<CallToolResult> {
        let result = self.client.get_function_details(functions).map_err(|e| {
            McpError::internal_error(format!("Failed to get function details: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        title = "Execute Code",
        description = "Execute TypeScript code that calls namespaced functions. USE THIS LAST after list_functions() and get_function_details().

        To minimize tokens:
        - Filter/map/reduce data IN CODE before returning
        - Only return specific fields you need (e.g., return {id: result.id, count: items.length})

        REQUIRED CODE STRUCTURE:
        async function run() {
            // Your code here
            // Call namespace.functionName() - MUST include namespace prefix
            // Process data here to minimize return size
            return yourResult;
        }

        IMPORTANT RULES:
        - Functions MUST be called as 'Namespace.functionName' (e.g., 'Notion.apiPostSearch')
        - Only functions from list_functions() are available - no fetch(), fs, or other APIs
        - Variables don't persist between execute() calls - return or log anything you need later
        - Add console.log() statements between API calls to track progress if errors occur

        RETURN TYPE NOTE:
        - Functions without output schemas show Promise<any> as return type
        - The actual runtime value is already a parsed JavaScript object, NOT a JSON string
        - Access properties directly (e.g., result.data) or inspect with console.log() first
        - If you see 'Promise<any>', the structure is unknown - log it to see what's returned
        "
    )]
    async fn execute(
        &self,
        Parameters(ExecuteInput { code }): Parameters<ExecuteInput>,
    ) -> McpResult<CallToolResult> {
        debug!(
            code_from_llm = %code,
            code_length = code.len(),
            "Received code to execute"
        );

        let result = self.client.execute(&code).await.map_err(|e| {
            error!("Execution failed: {e}");
            McpError::internal_error(format!("Execution failed: {e}"), None)
        })?;

        if result.success {
            debug!("Sandbox execution completed successfully");
        } else {
            warn!("Sandbox execution failed: {:?}", result.stderr);
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

#[allow(clippy::doc_markdown)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct ExecuteInput {
    /// Typescript code to execute.
    ///
    /// REQUIRED FORMAT:
    /// async function ``run()`` {
    ///   // YOUR CODE GOES HERE e.g. const result = await ``Namespace.method();``
    ///   // ALWAYS RETURN THE RESULT e.g. return result;
    /// }
    ///
    /// IMPORTANT: Your code should ONLY contain the function definition.
    /// The sandbox automatically calls run() and exports the result.
    ///
    pub code: String,
}

impl ServerHandler for PtcxTools {
    fn get_info(&self) -> ServerInfo {
        let config = self.config();
        let upstream = self.upstream();

        let default_description = format!(
            "This server provides tools to explore SDK functions and execute SDK scripts for the following services: {}",
            upstream
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
        );

        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: config.name.clone(),
                title: Some(config.name.clone()),
                version: config.version.clone(),
                ..Default::default()
            },
            instructions: Some(config.description.clone().unwrap_or(default_description)),
        }
    }

    #[instrument(skip_all, fields(mcp.method = "tools/list", mcp.id = %ctx.id))]
    async fn list_tools(
        &self,
        _req: Option<PaginatedRequestParam>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let start = std::time::Instant::now();
        let res = ListToolsResult::with_all_items(self.tool_router.list_all());
        let latency = start.elapsed();
        info!(
            tools.length = res.tools.len(),
            tools.next_cursor = res.next_cursor.is_some(),
            latency_ms = latency.as_millis(),
            "tools/list"
        );

        // Record metrics
        if let Some(metrics) = mcp_tool_metrics() {
            metrics
                .list_duration
                .record(latency.as_secs_f64() * 1000.0, &[]);
        }

        Ok(res)
    }

    #[instrument(skip_all, fields(mcp.method = "tools/call", mcp.id = %ctx.id, mcp.tool.name = %req.name))]
    async fn call_tool(
        &self,
        req: CallToolRequestParam,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let start = std::time::Instant::now();
        let tool_name = req.name.clone();

        let tcc = ToolCallContext::new(self, req, ctx);
        let res = self.tool_router.call(tcc).await;

        let latency = start.elapsed();
        let is_error = res
            .as_ref()
            .map(|r| r.is_error.unwrap_or_default())
            .unwrap_or(true);

        // Record metrics
        if let Some(metrics) = mcp_tool_metrics() {
            let attrs = vec![
                KeyValue::new("tool_name", tool_name.clone()),
                KeyValue::new("status", if is_error { "error" } else { "success" }),
            ];

            metrics
                .call_duration
                .record(latency.as_secs_f64() * 1000.0, &attrs);
            metrics.calls_total.add(1, &attrs);

            if is_error {
                metrics.errors_total.add(
                    1,
                    &[
                        KeyValue::new("tool_name", tool_name.clone()),
                        KeyValue::new("error_type", "tool_error"),
                    ],
                );
            }
        }

        let res = res?;

        info!(
            tool.result.is_error = res.is_error.unwrap_or_default(),
            tool.result.has_structured_content = res.structured_content.is_some(),
            latency_ms = latency.as_millis(),
            "tools/call - {tool_name}"
        );

        Ok(res)
    }
}
