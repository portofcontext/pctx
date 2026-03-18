use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use pctx_code_mode::{
    CodeMode,
    config::{Config, ToolDisclosure},
    descriptions,
    model::{ExecuteBashInput, ExecuteTypescriptInput, GetFunctionDetailsInput},
    registry::{McpConnectionPool, PctxRegistry, RegistryAction},
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
use uuid::Uuid;

type McpResult<T> = Result<T, rmcp::ErrorData>;

#[derive(Clone)]
pub(crate) struct PctxMcpService {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: Option<String>,
    pub(crate) code_mode: CodeMode,
    pub(crate) disclosure: ToolDisclosure,
    pub(crate) tool_router: ToolRouter<PctxMcpService>,
    /// Connection pools keyed by MCP session ID. Allows stateful upstream MCP
    /// connections (e.g. LSP) to survive across `execute_typescript` calls
    /// within the same session.
    pub(crate) pool_cache: Arc<Mutex<HashMap<String, Arc<McpConnectionPool>>>>,
    global_session_id: Option<Uuid>,
}

#[tool_router]
impl PctxMcpService {
    pub(crate) fn new(cfg: &Config, code_mode: CodeMode) -> Self {
        Self {
            name: cfg.name.clone(),
            version: cfg.version.clone(),
            description: cfg.description.clone(),
            code_mode,
            disclosure: cfg.disclosure,
            tool_router: Self::tool_router(),
            pool_cache: Arc::new(Mutex::new(HashMap::new())),
            global_session_id: None,
        }
    }

    pub(crate) fn with_global_session_id(mut self, id: Option<Uuid>) -> Self {
        self.global_session_id = id;
        self
    }

    fn get_session_id(&self, ctx: RequestContext<RoleServer>) -> Option<String> {
        let header_id = ctx
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|parts| parts.headers.get("mcp-session-id"))
            .and_then(|v| v.to_str().ok().map(String::from));

        if header_id.is_some() {
            header_id
        } else {
            self.global_session_id.map(|g| g.to_string())
        }
    }

    pub(crate) fn list_filtered_tools(&self) -> ListToolsResult {
        let original_list_tools = ListToolsResult::with_all_items(self.tool_router.list_all());
        let mut filtered = original_list_tools.clone();
        filtered.tools.clear();

        if matches!(self.disclosure, ToolDisclosure::Sidecar) {
            // add upstream tools to list of tools
            for (_, tool_set) in self.code_mode.server_tool_sets() {
                filtered.tools.extend(tool_set.tools.iter().map(|t| {
                    let input_schema: Option<rmcp::model::JsonObject> =
                        serde_json::from_value(json!(t.input_schema.clone()))
                            .ok()
                            .flatten();
                    let output_schema: Option<Arc<rmcp::model::JsonObject>> =
                        serde_json::from_value(json!(t.output_schema.clone()))
                            .ok()
                            .flatten();

                    let mut tool = rmcp::model::Tool::new(
                        t.id(tool_set.name.as_deref()),
                        t.description.clone().unwrap_or_default(),
                        input_schema.unwrap_or_default(),
                    );
                    tool.output_schema = output_schema;
                    tool
                }));
            }
        }

        // dynamically add descriptions based on tool disclosure
        let overrides = ToolOverride::for_disclosure(self.disclosure);
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
        session_id: Option<&str>,
        mut req: CallToolRequestParams,
    ) -> McpResult<CallToolResult> {
        let registry = self.get_pctx_registry(session_id).await?;

        if let Some(RegistryAction::Mcp(mcp_tool_id)) = registry.get(&req.name) {
            let server = self
                .code_mode
                .servers()
                .iter()
                .find(|s| s.name == mcp_tool_id.sever_name)
                .ok_or(rmcp::ErrorData::invalid_params("tool not found", None))?;
            let (client, _cached) = registry.pool().get_or_connect(server).await.map_err(|e| {
                rmcp::ErrorData::invalid_request(
                    format!(
                        "failed connecting to upstream MCP at `{}`: {e}",
                        server.display_target()
                    ),
                    None,
                )
            })?;
            req.name = mcp_tool_id.tool_name.into();

            let call_tool_res = client.call_tool(req).await.map_err(service_error_to_mcp);

            self.cache_pool(session_id, registry.pool())?;

            call_tool_res
        } else {
            Err(rmcp::ErrorData::invalid_params("tool not found", None))
        }
    }

    #[tool(title = "List Functions")]
    async fn list_functions(&self) -> McpResult<CallToolResult> {
        let listed = self.code_mode.list_functions();

        Ok(CallToolResult::success(vec![Content::text(listed.code)]))
    }

    #[tool(title = "Get Function Details")]
    async fn get_function_details(
        &self,
        Parameters(input): Parameters<GetFunctionDetailsInput>,
    ) -> McpResult<CallToolResult> {
        let details = self.code_mode.get_function_details(input);

        Ok(CallToolResult::success(vec![Content::text(details.code)]))
    }

    #[tool(title = "Execute Bash")]
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

        Ok(CallToolResult::success(vec![Content::text(
            execution_output.to_string(),
        )]))
    }

    #[tool(title = "Execute Typescript Code")]
    async fn execute_typescript(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(input): Parameters<ExecuteTypescriptInput>,
    ) -> McpResult<CallToolResult> {
        self.handle_execute_typescript(input, self.get_session_id(ctx))
            .await
    }

    async fn get_pctx_registry(&self, session_id: Option<&str>) -> McpResult<PctxRegistry> {
        let mut registry = self.code_mode.default_registry().map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to default pctx registry: {e}"), None)
        })?;

        if let Some(sid) = session_id {
            let cache_entry = self
                .pool_cache
                .lock()
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed obtaining lock on MPC connection pool cache: {e}"),
                        None,
                    )
                })?
                .get(sid)
                .cloned();

            if let Some(cached_pool) = cache_entry {
                info!(session_id =% sid, "MCP pool cache hit");

                registry = registry.with_pool(cached_pool);
            } else {
                // Pre-warm connections on the outer async runtime so the spawned tasks
                // are owned by it rather than the short-lived per-execution runtime.
                // The inner runtime then hits the fast path in get_or_connect (already
                // live) and never needs to spawn new connection tasks itself.
                info!(session_id =% sid, "MCP pool cache missed, prewarming connections...");
                registry.prewarm_pool().await.map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed pre-warming pctx MCP connection pool: {e}"),
                        None,
                    )
                })?;
            }
        } else {
            debug!("No session ID present, skipping MCP pool cache");
        }

        Ok(registry)
    }

    fn cache_pool(&self, session_id: Option<&str>, pool: Arc<McpConnectionPool>) -> McpResult<()> {
        if let Some(sid) = session_id {
            let mut cache = self.pool_cache.lock().map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("Failed obtaining lock on MPC connection pool cache: {e}"),
                    None,
                )
            })?;
            cache.insert(sid.into(), pool);
            info!(session_id =% sid, "MCP connection pool cached");
        } else {
            info!("Not caching MCP connection pool - no session_id");
        }

        Ok(())
    }

    async fn handle_execute_typescript(
        &self,
        input: ExecuteTypescriptInput,
        session_id: Option<String>,
    ) -> McpResult<CallToolResult> {
        // Capture current tracing context to propagate to spawned thread
        let current_span = tracing::Span::current();

        let registry = self.get_pctx_registry(session_id.as_deref()).await?;
        let code_mode = self.code_mode.clone();
        let code = input.code;
        let style = self.disclosure;

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
                    .execute_typescript(&code, style, Some(registry))
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

        self.cache_pool(session_id.as_deref(), execution_output.registry.pool())?;

        Ok(CallToolResult::success(vec![Content::text(
            execution_output.markdown(),
        )]))
    }
}

impl ServerHandler for PctxMcpService {
    fn get_info(&self) -> ServerInfo {
        let available_namespaces = format!(
            "This server provides tools to explore SDK functions and execute SDK scripts for the following services: {}",
            self.code_mode
                .tool_sets()
                .iter()
                .map(|s| s.pascal_namespace())
                .collect::<Vec<String>>()
                .join(", ")
        );

        let workflow =
            pctx_code_mode::descriptions::workflow::get_workflow_description(self.disclosure);

        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_server_info(
                Implementation::new(self.name.clone(), self.version.clone())
                    .with_title(self.name.clone()),
            )
            .with_instructions(
                self.description
                    .clone()
                    .unwrap_or(format!("{available_namespaces}\n{workflow}")),
            )
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
            if matches!(self.disclosure, ToolDisclosure::Sidecar)
                && tool_name != "execute_typescript"
            {
                // call tool directly
                debug!("Calling tool directly in sidecar style");
                let session_id = self.get_session_id(ctx);
                self.handle_direct_tool_call(session_id.as_deref(), req)
                    .await
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
    fn for_disclosure(disclosure: ToolDisclosure) -> HashMap<String, Self> {
        let mut overrides = HashMap::new();

        // catalog only
        overrides.insert(
            "list_functions".into(),
            Self {
                enabled: matches!(disclosure, ToolDisclosure::Catalog),
                description: descriptions::tools::LIST_FUNCTIONS.into(),
            },
        );
        overrides.insert(
            "get_function_details".into(),
            Self {
                enabled: matches!(disclosure, ToolDisclosure::Catalog),
                description: descriptions::tools::GET_FUNCTION_DETAILS.into(),
            },
        );

        // fs only
        overrides.insert(
            "execute_bash".into(),
            Self {
                enabled: matches!(disclosure, ToolDisclosure::Filesystem),
                description: descriptions::tools::EXECUTE_BASH.into(),
            },
        );

        // execute_typescript
        overrides.insert(
            "execute_typescript".into(),
            Self {
                enabled: true,
                description: descriptions::tools::disclosure_execute_description(disclosure),
            },
        );

        overrides
    }
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
