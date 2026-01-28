use anyhow::Context;
use axum::{Json, extract::State, http::StatusCode};

use pctx_code_mode::{
    CodeMode,
    model::{
        CallbackConfig, GetFunctionDetailsInput, GetFunctionDetailsOutput, ListFunctionsOutput,
    },
};
use tracing::info;
use uuid::Uuid;

use crate::extractors::CodeModeSession;
use crate::model::{
    ApiError, ApiResult, CloseSessionResponse, CreateSessionResponse, ErrorCode, ErrorData,
    HealthResponse, RegisterMcpServersRequest, RegisterMcpServersResponse, RegisterToolsRequest,
    RegisterToolsResponse,
};
use crate::state::{AppState, backend::PctxSessionBackend};

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Create a new `CodeMode` session
#[utoipa::path(
    post,
    path = "/code-mode/session/create",
    tag = "CodeMode",
    responses(
        (status = 200, description = "Session created successfully", body = CreateSessionResponse),
        (status = 500, description = "Internal server error", body = ErrorData)
    )
)]
pub(crate) async fn create_session<B: PctxSessionBackend>(
    State(state): State<AppState<B>>,
) -> ApiResult<Json<CreateSessionResponse>> {
    let session_id = Uuid::new_v4();
    info!(
        session_id =? session_id,
        "Creating new CodeMode session"
    );

    let code_mode = CodeMode::default();
    state
        .backend
        .insert(session_id, code_mode)
        .await
        .context("Failed inserting code mode session into backend")?;

    info!(
        session_id =? session_id,
        "Created CodeMode session"
    );

    Ok(Json(CreateSessionResponse { session_id }))
}

/// Close a `CodeMode` session
#[utoipa::path(
    post,
    path = "/code-mode/session/close",
    tag = "CodeMode",
    params(
        ("x-code-mode-session" = String, Header, description = "Current code mode session")
    ),
    responses(
        (status = 200, description = "Session closed successfully", body = CloseSessionResponse),
        (status = 404, description = "Session not found", body = ErrorData),
        (status = 500, description = "Internal server error", body = ErrorData)
    )
)]
pub(crate) async fn close_session<B: PctxSessionBackend>(
    State(state): State<AppState<B>>,
    CodeModeSession(session_id): CodeModeSession,
) -> ApiResult<Json<CloseSessionResponse>> {
    info!(session_id =? session_id, "Closing CodeMode session");

    let existed = state
        .backend
        .delete(session_id)
        .await
        .context("Failed deleting code mode session from backend")?;

    if !existed {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            ErrorData {
                code: ErrorCode::InvalidSession,
                message: format!("Code Mode session {session_id} does not exist"),
                details: None,
            },
        ));
    }

    info!(session_id =? session_id, "Closed CodeMode session");

    Ok(Json(CloseSessionResponse { success: true }))
}

/// List all available code mode functions from both server and tool registrations
#[utoipa::path(
    post,
    path = "/code-mode/functions/list",
    tag = "CodeMode",
    params(
        ("x-code-mode-session" = String, Header, description = "Current code mode session")
    ),
    responses(
        (status = 200, description = "List of all code mode functions as source code & structured output", body = ListFunctionsOutput),
        (status = 500, description = "Internal server error", body = ErrorData)
    )
)]
pub(crate) async fn list_functions<B: PctxSessionBackend>(
    State(state): State<AppState<B>>,
    CodeModeSession(session_id): CodeModeSession,
) -> ApiResult<Json<ListFunctionsOutput>> {
    info!(session_id =? session_id, "Listing functions");

    let code_mode = state
        .backend
        .get(session_id)
        .await
        .context("Failed getting code mode session")?
        .ok_or(ApiError::new(
            StatusCode::NOT_FOUND,
            ErrorData {
                code: ErrorCode::InvalidSession,
                message: format!("Code Mode session {session_id} does not exist"),
                details: None,
            },
        ))?;

    let functions = code_mode.list_functions();

    Ok(Json(functions))
}

/// Get detailed information about a specific function
#[utoipa::path(
    post,
    path = "/code-mode/functions/details",
    tag = "CodeMode",
    params(
        ("x-code-mode-session" = String, Header, description = "Current code mode session")
    ),
    request_body = GetFunctionDetailsInput,
    responses(
        (status = 200, description = "Function details", body = GetFunctionDetailsOutput),
        (status = 404, description = "Function not found", body = ErrorData),
        (status = 500, description = "Internal server error", body = ErrorData)
    )
)]
pub(crate) async fn get_function_details<B: PctxSessionBackend>(
    State(state): State<AppState<B>>,
    CodeModeSession(session_id): CodeModeSession,
    Json(request): Json<GetFunctionDetailsInput>,
) -> ApiResult<Json<GetFunctionDetailsOutput>> {
    let requested_functions = request
        .functions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(", ");
    info!(
        session_id =? session_id,
        functions =? requested_functions,
        "Getting function details",
    );

    let code_mode = state.backend.get(session_id).await?.ok_or(ApiError::new(
        StatusCode::NOT_FOUND,
        ErrorData {
            code: ErrorCode::InvalidSession,
            message: format!("Code mode session {session_id} does not exist"),
            details: None,
        },
    ))?;

    let details = code_mode.get_function_details(request);

    Ok(Json(details))
}

/// Register tools that will be called via WebSocket callbacks
#[utoipa::path(
    post,
    path = "/register/tools",
    tag = "registration",
    params(
        ("x-code-mode-session" = String, Header, description = "Current code mode session")
    ),
    request_body = RegisterToolsRequest,
    responses(
        (status = 200, description = "Tools registered successfully", body = RegisterToolsResponse),
        (status = 404, description = "Session not found", body = ErrorData),
        (status = 500, description = "Internal server error", body = ErrorData)
    )
)]
pub(crate) async fn register_tools<B: PctxSessionBackend>(
    State(state): State<AppState<B>>,
    CodeModeSession(session_id): CodeModeSession,
    Json(request): Json<RegisterToolsRequest>,
) -> ApiResult<Json<RegisterToolsResponse>> {
    let tool_ids = request
        .tools
        .iter()
        .map(CallbackConfig::id)
        .collect::<Vec<_>>();
    info!(
        session_id =? session_id,
        tools =? &tool_ids,
        "Registering tools...",
    );

    let mut code_mode = state
        .backend
        .get(session_id)
        .await
        .context("Failed getting codemode session from backend")?
        .ok_or(ApiError::new(
            StatusCode::NOT_FOUND,
            ErrorData {
                code: ErrorCode::InvalidSession,
                message: format!("Code mode session {session_id} does not exist"),
                details: None,
            },
        ))?;
    code_mode
        .add_callbacks(&request.tools)
        .context("Failed adding callbacks")?;

    // Update the backend with the modified CodeMode
    state.backend.update(session_id, code_mode).await?;

    info!(
        session_id =? session_id,
        tools =? &tool_ids,
        "Registered tools",
    );

    Ok(Json(RegisterToolsResponse {
        registered: request.tools.len(),
    }))
}

/// Register MCP servers dynamically at runtime
#[utoipa::path(
    post,
    path = "/register/servers",
    tag = "registration",
    params(
        ("x-code-mode-session" = String, Header, description = "Current code mode session")
    ),
    request_body = RegisterMcpServersRequest,
    responses(
        (status = 200, description = "MCP servers registration result", body = RegisterMcpServersResponse),
        (status = 500, description = "Internal server error", body = ErrorData)
    )
)]
pub(crate) async fn register_servers<B: PctxSessionBackend>(
    State(state): State<AppState<B>>,
    CodeModeSession(session_id): CodeModeSession,
    Json(request): Json<RegisterMcpServersRequest>,
) -> ApiResult<Json<RegisterMcpServersResponse>> {
    info!(
        "Registering {} MCP servers in session {session_id}",
        request.servers.len()
    );

    let mut code_mode = state
        .backend
        .get(session_id)
        .await
        .context("Failed getting code mode session from backend")?
        .ok_or(ApiError::new(
            StatusCode::NOT_FOUND,
            ErrorData {
                code: ErrorCode::InvalidSession,
                message: format!("Code mode session {session_id} does not exist"),
                details: None,
            },
        ))?;

    // Use parallel server registration with conversion function
    code_mode
        .add_servers(&request.servers, 30)
        .await
        .context("Failed adding servers")?;

    // Update the backend with the modified CodeMode
    state
        .backend
        .update(session_id, code_mode)
        .await
        .context("Failed updating code mode session in backend")?;

    info!(
        session_id =% session_id,
        registered =% request.servers.len(),
        "Registered MCP servers",
    );

    Ok(Json(RegisterMcpServersResponse {
        registered: request.servers.len(),
        failed: vec![],
    }))
}
