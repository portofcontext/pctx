use pctx_code_mode::{
    CodeMode,
    model::{
        ExecuteInput, ExecuteOutput, GetFunctionDetailsInput, GetFunctionDetailsOutput,
        ListFunctionsOutput,
    },
};
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParam, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParam, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_router,
};
use serde_json::json;
use tracing::{error, info, instrument};

// Metrics removed - will be added via telemetry support later

type McpResult<T> = Result<T, rmcp::ErrorData>;

#[derive(Clone)]
pub(crate) struct PctxMcpService {
    name: String,
    version: String,
    description: Option<String>,
    code_mode: CodeMode,
    tool_router: ToolRouter<PctxMcpService>,
}

#[tool_router]
impl PctxMcpService {
    pub(crate) fn new(cfg: &pctx_config::Config, code_mode: CodeMode) -> Self {
        Self {
            name: cfg.name.clone(),
            version: cfg.version.clone(),
            description: cfg.description.clone(),
            code_mode,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        title = "List Functions",
        description = "ALWAYS USE THIS TOOL FIRST to list all available functions organized by namespace.

        WORKFLOW:
        1. Start here - Call this tool to see what functions are available
        2. Then call get_function_details() for specific functions you need to understand
        3. Finally call execute() to run your TypeScript code

        This returns function signatures without full details.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ListFunctionsOutput>()
    )]
    async fn list_functions(&self) -> McpResult<CallToolResult> {
        let listed = self.code_mode.list_functions();
        let mut res = CallToolResult::success(vec![Content::text(&listed.code)]);
        res.structured_content = Some(json!(listed));

        Ok(res)
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
        - Don't use JSON.parse() on the results - they're already JavaScript objects",
        output_schema = rmcp::handler::server::tool::schema_for_type::<GetFunctionDetailsOutput>()
    )]
    async fn get_function_details(
        &self,
        Parameters(input): Parameters<GetFunctionDetailsInput>,
    ) -> McpResult<CallToolResult> {
        let details = self.code_mode.get_function_details(input);
        let mut res = CallToolResult::success(vec![Content::text(&details.code)]);
        res.structured_content = Some(json!(details));

        Ok(res)
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
        ",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ExecuteOutput>()
    )]
    async fn execute(
        &self,
        Parameters(input): Parameters<ExecuteInput>,
    ) -> McpResult<CallToolResult> {
        // Capture current tracing context to propagate to spawned thread
        let current_span = tracing::Span::current();

        let code_mode = self.code_mode.clone();
        let code = input.code;

        let execution_output = tokio::task::spawn_blocking(move || -> Result<_, anyhow::Error> {
            // Enter the captured span context in the new thread
            let _guard = current_span.enter();

            // Create a new current-thread runtime for Deno ops that use deno_unsync
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to create runtime: {e}"))?;

            rt.block_on(async {
                code_mode
                    .execute(&code, None)
                    .await
                    .map_err(|e| anyhow::anyhow!("Execution error: {e}"))
            })
        })
        .await
        .map_err(|e| {
            error!("Task join failed: {e}");
            rmcp::ErrorData::internal_error(format!("Task join failed: {e}"), None)
        })?
        .map_err(|e| {
            error!("Sandbox execution error: {e}");
            rmcp::ErrorData::internal_error(format!("Execution failed: {e}"), None)
        })?;

        let mut res = CallToolResult::success(vec![Content::text(execution_output.markdown())]);
        res.structured_content = Some(json!(execution_output));

        Ok(res)
    }
}

impl ServerHandler for PctxMcpService {
    fn get_info(&self) -> ServerInfo {
        let default_description = format!(
            "This server provides tools to explore SDK functions and execute SDK scripts for the following services: {}",
            self.code_mode
                .tool_sets
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<String>>()
                .join(", ")
        );

        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: self.name.clone(),
                title: Some(self.name.clone()),
                version: self.version.clone(),
                ..Default::default()
            },
            instructions: Some(self.description.clone().unwrap_or(default_description)),
        }
    }

    #[instrument(skip_all, fields(mcp.method = "tools/list", mcp.id = %ctx.id))]
    async fn list_tools(
        &self,
        _req: Option<PaginatedRequestParam>,
        ctx: RequestContext<RoleServer>,
    ) -> McpResult<ListToolsResult> {
        let start = std::time::Instant::now();
        let res = ListToolsResult::with_all_items(self.tool_router.list_all());
        let latency = start.elapsed();
        info!(
            tools.length = res.tools.len(),
            tools.next_cursor = res.next_cursor.is_some(),
            latency_ms = latency.as_millis(),
            "tools/list"
        );

        // Metrics disabled for now
        let _ = latency;

        Ok(res)
    }

    #[instrument(skip_all, fields(mcp.method = "tools/call", mcp.id = %ctx.id, mcp.tool.name = %req.name))]
    async fn call_tool(
        &self,
        req: CallToolRequestParam,
        ctx: RequestContext<RoleServer>,
    ) -> McpResult<CallToolResult> {
        let start = std::time::Instant::now();
        let tool_name = req.name.clone();

        let tcc = ToolCallContext::new(self, req, ctx);
        let res = self.tool_router.call(tcc).await;

        let latency = start.elapsed();
        let is_error = res
            .as_ref()
            .map(|r| r.is_error.unwrap_or_default())
            .unwrap_or(true);

        // Metrics disabled for now
        let _ = (is_error, latency);

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
