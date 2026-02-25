use std::collections::HashMap;

use pctx_code_mode::{
    CodeMode, PctxRegistry, RegistryAction,
    model::{
        DisclosureStyle, ExecuteBashInput, ExecuteInput, ExecuteOutput, GetFunctionDetailsInput,
        GetFunctionDetailsOutput, ListFunctionsOutput,
    },
    tool_descriptions,
};
use rmcp::{
    RoleServer, ServerHandler, ServiceError,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_router,
};
use serde_json::json;
use tracing::{debug, error, info, instrument};

// Metrics removed - will be added via telemetry support later

type McpResult<T> = Result<T, rmcp::ErrorData>;

#[derive(Clone)]
pub(crate) struct PctxMcpService {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: Option<String>,
    pub(crate) code_mode: CodeMode,
    pub(crate) disclosure_style: DisclosureStyle,
    pub(crate) tool_router: ToolRouter<PctxMcpService>,
}

#[tool_router]
impl PctxMcpService {
    pub(crate) fn new(cfg: &pctx_config::Config, code_mode: CodeMode) -> Self {
        Self {
            name: cfg.name.clone(),
            version: cfg.version.clone(),
            description: cfg.description.clone(),
            code_mode,
            disclosure_style: DisclosureStyle::Sidecar, // TODO: from cfg
            tool_router: Self::tool_router(),
        }
    }

    pub(crate) fn list_filtered_tools(&self) -> ListToolsResult {
        let original_list_tools = ListToolsResult::with_all_items(self.tool_router.list_all());
        let mut filtered = original_list_tools.clone();
        filtered.tools.clear();

        if matches!(self.disclosure_style, DisclosureStyle::Sidecar) {
            // add upstream tools to list of tools
            for (_, tool_set) in self.code_mode.server_tool_sets() {
                filtered.tools.extend(tool_set.tools.iter().map(|t| {
                    let input_schema =
                        serde_json::from_value(json!(t.input_schema.clone())).unwrap();
                    let output_schema =
                        serde_json::from_value(json!(t.output_schema.clone())).unwrap();
                    rmcp::model::Tool {
                        name: t.id(tool_set.name.as_deref()).into(),
                        description: t.description.clone().map(|d| d.into()),
                        input_schema,
                        output_schema,
                        title: None,
                        annotations: None,
                        icons: None,
                        meta: None,
                    }
                }));
            }
        }

        // dynamically add descriptions based on style
        let overrides = ToolOverride::for_style(self.disclosure_style);
        for mut tool in original_list_tools.tools {
            if let Some(o) = overrides.get(&tool.name.to_string()) {
                if !o.enabled {
                    continue;
                }
                tool.description = Some(o.description.clone().into());
            }

            filtered.tools.push(tool)
        }

        filtered
    }

    pub(crate) async fn handle_direct_tool_call(
        &self,
        mut req: CallToolRequestParams,
    ) -> McpResult<CallToolResult> {
        let mut registry = PctxRegistry::default();
        self.code_mode
            .add_mcp_servers_to_registry(&mut registry)
            .map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("failed building internal MCP registry: {e}"),
                    None,
                )
            })?;

        if let Some(RegistryAction::Mcp(mcp_tool_id)) = registry.get(&req.name) {
            let server = self
                .code_mode
                .servers()
                .iter()
                .find(|s| s.name == mcp_tool_id.sever_name)
                .ok_or(rmcp::ErrorData::invalid_params("tool not found", None))?;
            let client = server.connect().await.map_err(|e| {
                rmcp::ErrorData::invalid_request(
                    format!(
                        "failed connecting to upstream MCP at `{}`: {e}",
                        server.display_target()
                    ),
                    None,
                )
            })?;
            req.name = mcp_tool_id.tool_name.into();

            client.call_tool(req).await.map_err(service_error_to_mcp)
        } else {
            Err(rmcp::ErrorData::invalid_params("tool not found", None))
        }
    }

    #[tool(
                                    title = "List Functions",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ListFunctionsOutput>()
    )]
    async fn list_functions(&self) -> McpResult<CallToolResult> {
        let listed = self.code_mode.list_functions();
        let res = success_with_structure(&listed.code, &listed);

        Ok(res)
    }

    #[tool(
        title = "Get Function Details",
        output_schema = rmcp::handler::server::tool::schema_for_type::<GetFunctionDetailsOutput>()
    )]
    async fn get_function_details(
        &self,
        Parameters(input): Parameters<GetFunctionDetailsInput>,
    ) -> McpResult<CallToolResult> {
        let details = self.code_mode.get_function_details(input);
        let res = success_with_structure(&details.code, &details);

        Ok(res)
    }

    #[tool(
        title = "Execute Bash",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ExecuteOutput>()
    )]
    async fn execute_bash(
        &self,
        Parameters(input): Parameters<ExecuteBashInput>,
    ) -> McpResult<CallToolResult> {
        // Capture current tracing context to propagate to spawned thread
        let current_span = tracing::Span::current();

        let code_mode = self.code_mode.clone();
        let command = input.command;

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
                    .execute_bash(&command)
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

    #[tool(
        title = "Execute Typescript Code",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ExecuteOutput>()
    )]
    async fn execute_typescript(
        &self,
        Parameters(input): Parameters<ExecuteInput>,
    ) -> McpResult<CallToolResult> {
        // Capture current tracing context to propagate to spawned thread
        let current_span = tracing::Span::current();

        let code_mode = self.code_mode.clone();
        let code = input.code;
        let style = self.disclosure_style;

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
                    .execute_typescript(&code, style, None)
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
                .tool_sets()
                .iter()
                .map(|s| s.pascal_namespace())
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
        _req: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> McpResult<ListToolsResult> {
        let start = std::time::Instant::now();
        let filtered_tools = self.list_filtered_tools();

        let latency = start.elapsed();
        info!(
            tools.length = filtered_tools.tools.len(),
            tools.next_cursor = filtered_tools.next_cursor.is_some(),
            latency_ms = latency.as_millis(),
            "tools/list"
        );

        Ok(filtered_tools)
    }

    #[instrument(skip_all, fields(mcp.method = "tools/call", mcp.id = %ctx.id, mcp.tool.name = %req.name))]
    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> McpResult<CallToolResult> {
        let start = std::time::Instant::now();
        let tool_name = req.name.clone();

        let res: Result<CallToolResult, rmcp::ErrorData> =
            if matches!(self.disclosure_style, DisclosureStyle::Sidecar)
                && tool_name != "execute_typescript"
            {
                // call tool directly
                debug!("Calling tool directly in sidecar style");
                self.handle_direct_tool_call(req).await
            } else {
                let tcc = ToolCallContext::new(self, req, ctx);
                self.tool_router.call(tcc).await
            };

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

struct ToolOverride {
    enabled: bool,
    description: String,
}
impl ToolOverride {
    fn for_style(style: DisclosureStyle) -> HashMap<String, Self> {
        let mut overrides = HashMap::new();

        // catalog only
        overrides.insert(
            "list_functions".into(),
            Self {
                enabled: matches!(style, DisclosureStyle::Catalog),
                description: tool_descriptions::LIST_FUNCTIONS.into(),
            },
        );
        overrides.insert(
            "get_function_details".into(),
            Self {
                enabled: matches!(style, DisclosureStyle::Catalog),
                description: tool_descriptions::GET_FUNCTION_DETAILS.into(),
            },
        );

        // fs only
        overrides.insert(
            "execute_bash".into(),
            Self {
                enabled: matches!(style, DisclosureStyle::Filesystem),
                description: tool_descriptions::EXECUTE_BASH.into(),
            },
        );

        // execute_typescript
        overrides.insert(
            "execute_typescript".into(),
            Self {
                enabled: true,
                description: style.execute_description(),
            },
        );

        overrides
    }
}

fn success_with_structure<V: serde::Serialize>(text: &str, structured: V) -> CallToolResult {
    let mut res = CallToolResult::success(vec![Content::text(text)]);
    res.structured_content = Some(json!(structured));

    res
}

fn service_error_to_mcp(e: ServiceError) -> rmcp::ErrorData {
    match e {
        ServiceError::McpError(mcp_err) => mcp_err,
        ServiceError::TransportClosed => rmcp::ErrorData::internal_error("transport closed", None),
        ServiceError::TransportSend(err) => rmcp::ErrorData::internal_error(err.to_string(), None),
        ServiceError::UnexpectedResponse => {
            rmcp::ErrorData::internal_error("unexpected response type", None)
        }
        ServiceError::Cancelled { reason } => {
            rmcp::ErrorData::internal_error(reason.unwrap_or_else(|| "cancelled".to_string()), None)
        }
        ServiceError::Timeout { timeout } => {
            rmcp::ErrorData::internal_error(format!("request timeout after {timeout:?}"), None)
        }
        _ => rmcp::ErrorData::internal_error(e.to_string(), None),
    }
}
