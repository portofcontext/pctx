use std::{collections::HashMap, time::Duration};

use axum::{Json, http::StatusCode, response::IntoResponse};
use pctx_code_mode::{config, model::ExecuteTypescriptOutput};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use utoipa::ToSchema;
use uuid::Uuid;

// ----------- REST API STRUCTS -----------

pub(crate) type ApiResult<T> = Result<T, ApiError>;

pub(crate) struct ApiError {
    pub(crate) code: StatusCode,
    pub(crate) data: ErrorData,
    pub(crate) internal: String,
}
impl ApiError {
    pub(crate) fn new(code: StatusCode, data: ErrorData) -> Self {
        let internal = format!("{}, details: {:?}", &data.message, data.details);
        Self {
            code,
            data,
            internal,
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        ApiError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            data: ErrorData {
                code: ErrorCode::Internal,
                message: "Internal error".into(),
                details: None,
            },
            internal: format!("{value}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        if self.code.is_server_error() {
            error!("Server Error: {}", self.internal);
        } else {
            warn!("Returning API error: {}", self.internal);
        }

        (self.code, Json(self.data)).into_response()
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorData {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidSession,
    Internal,
    Execution,
}

/// Request to register tools
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterToolsRequest {
    pub tools: Vec<pctx_code_mode::model::CallbackConfig>,
}

/// Response to registering tools.
///
/// `failed` lists tools that could not be registered at all (name clash,
/// unparseable schema); the rest of the batch still registers. A tool whose
/// schema our codegen can't express is registered with a permissive `any`
/// signature and reported in `warnings`, not failed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterToolsResponse {
    pub registered: usize,
    #[serde(default)]
    pub failed: Vec<pctx_code_mode::model::FailedCallback>,
    /// Tools that registered in a degraded form (e.g. types fell back to `any`).
    #[serde(default)]
    pub warnings: Vec<pctx_code_mode::model::CallbackWarning>,
}

/// Request to register MCP servers
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterMcpServersRequest {
    #[schema(value_type = Vec<serde_json::Value>)]
    pub servers: Vec<config::server::ServerConfig>,
}

/// Response after registering MCP servers
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterMcpServersResponse {
    pub registered: usize,
    pub failed: Vec<String>,
}

/// Response after creating a new `CodeMode` session
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSessionResponse {
    #[schema(value_type = String)]
    pub session_id: Uuid,
}
/// Response after closing a `CodeMode` session
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CloseSessionResponse {
    pub success: bool,
}

// ----------- Websocket JRPC Message structs -----------

pub type WsJsonRpcMessage = rmcp::model::JsonRpcMessage<PctxJsonRpcRequest, PctxJsonRpcResponse>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum PctxJsonRpcRequest {
    #[serde(alias = "execute_code")]
    #[serde(rename = "execute_typescript")]
    ExecuteTypescript { params: ExecuteTypescriptParams },
    #[serde(rename = "execute_tool")]
    ExecuteTool { params: ExecuteToolParams },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolParams {
    pub namespace: Option<String>,
    pub name: String,
    pub args: Option<serde_json::Value>,
}

impl ExecuteToolParams {
    /// Registry id of the tool (`namespace__name`), matching
    /// [`pctx_code_mode::model::CallbackConfig::id`].
    pub fn tool_id(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{ns}__{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// Timeout applied to a single tool call when the request specifies none.
pub const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 30;
/// Upper bound on a client-supplied tool call timeout.
///
/// Each in-flight call holds a blocking thread, so an unbounded value lets a
/// client pin one indefinitely.
pub const MAX_TOOL_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteTypescriptParams {
    pub code: String,
    #[serde(default)]
    pub disclosure: config::ToolDisclosure,
    /// Timeout applied to every tool call made by this execution, in seconds.
    ///
    /// Defaults to [`DEFAULT_TOOL_TIMEOUT_SECS`]. This bounds a single call, not
    /// the execution as a whole: code making N sequential calls can run for N
    /// times this value.
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
    /// Per-tool overrides of `tool_timeout_secs`, keyed by tool id
    /// (`namespace__name`, or just `name` when the tool has no namespace).
    ///
    /// Ids with no registered tool are ignored.
    #[serde(default)]
    pub tool_timeout_overrides: HashMap<String, u64>,
}

impl ExecuteTypescriptParams {
    /// Resolves the timeout for a tool: override, then request default, then
    /// [`DEFAULT_TOOL_TIMEOUT_SECS`] — clamped to [`MAX_TOOL_TIMEOUT_SECS`].
    pub fn tool_timeout(&self, tool_id: &str) -> Duration {
        let secs = self
            .tool_timeout_overrides
            .get(tool_id)
            .copied()
            .or(self.tool_timeout_secs)
            .unwrap_or(DEFAULT_TOOL_TIMEOUT_SECS);

        Duration::from_secs(secs.clamp(1, MAX_TOOL_TIMEOUT_SECS))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PctxJsonRpcResponse {
    ExecuteCode(ExecuteTypescriptOutput),
    ExecuteTool(ExecuteToolResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolResult {
    pub output: Option<serde_json::Value>,
}
