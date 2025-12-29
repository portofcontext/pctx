#![allow(clippy::needless_for_each)] // Caused by #[derive(OpenApi)]

use anyhow::Result;
use axum::{
    Router,
    routing::{get, post},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    AppState, PctxSessionBackend,
    model::{
        CloseSessionResponse, CreateSessionResponse, ErrorData, HealthResponse, McpServerConfig,
        RegisterMcpServersRequest, RegisterMcpServersResponse, RegisterToolsRequest,
        RegisterToolsResponse,
    },
    routes, websocket,
};
use pctx_code_mode::model::{
    CallbackConfig, FunctionDetails, GetFunctionDetailsInput, GetFunctionDetailsOutput,
    ListFunctionsOutput, ListedFunction,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::health,
        routes::create_session,
        routes::close_session,
        routes::list_functions,
        routes::get_function_details,
        routes::register_tools,
        routes::register_servers,
    ),
    components(
        schemas(
            HealthResponse,
            // Session management
            CreateSessionResponse,
            CloseSessionResponse,
            // List functions
            ListFunctionsOutput,
            ListedFunction,
            // Get function details
            GetFunctionDetailsInput,
            GetFunctionDetailsOutput,
            FunctionDetails,
            // Tool registration
            RegisterToolsRequest,
            CallbackConfig,
            RegisterToolsResponse,
            // Server registration
            RegisterMcpServersRequest,
            McpServerConfig,
            RegisterMcpServersResponse,
            // Common
            ErrorData
        )
    ),
    tags(
        (name = "tools", description = "Tool management and execution endpoints"),
        (name = "health", description = "Health check endpoints")
    ),
    info(
        title = "PCTX Agent Server API",
        version = "0.1.0",
        description = "REST API for PCTX agent mode - dynamic tool and MCP server registration with code execution",
    )
)]
pub struct ApiDoc;

/// Start the agent server
///
/// # Errors
///
/// This function will return an error if axum fails binding to the provided host/port
pub async fn start_server<B: PctxSessionBackend>(
    host: &str,
    port: u16,
    state: AppState<B>,
) -> Result<()> {
    let app = create_router(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("   PCTX Agent Server listening on http://{addr}");
    info!("   OpenAPI documentation: http://{addr}/swagger-ui/");
    info!("");
    info!("Use REST API to register tools and MCP servers dynamically.");
    info!("WebSocket endpoint at ws://{addr}/ws for tool callbacks.",);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Create the Axum router with all routes
pub fn create_router<B: PctxSessionBackend>(state: AppState<B>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(routes::health))
        // Session management
        .route("/code-mode/session/create", post(routes::create_session))
        .route("/code-mode/session/close", post(routes::close_session))
        // Tools endpoints
        .route("/code-mode/functions/list", post(routes::list_functions))
        .route(
            "/code-mode/functions/details",
            post(routes::get_function_details),
        )
        .route("/register/tools", post(routes::register_tools))
        .route("/register/servers", post(routes::register_servers))
        // WebSocket endpoint
        .route("/ws", get(websocket::ws_handler))
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Add state
        .with_state(state)
        // Add middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!("Shutdown signal received, cleaning up...");
}
