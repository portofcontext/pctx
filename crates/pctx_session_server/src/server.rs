#![allow(clippy::needless_for_each)] // Caused by #[derive(OpenApi)]

use anyhow::Result;
use axum::{
    Router,
    http::{HeaderValue, Method},
    routing::{get, post},
};
use opentelemetry::{global, trace::TraceContextExt};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    AppState, PctxSessionBackend,
    extractors::HeaderExtractor,
    model::{
        CloseSessionResponse, CreateSessionResponse, ErrorData, HealthResponse,
        RegisterMcpServersRequest, RegisterMcpServersResponse, RegisterToolsRequest,
        RegisterToolsResponse,
    },
    routes, websocket,
};
use pctx_code_mode::model::{
    CallbackConfig, ExecuteBashInput, ExecuteOutput, FunctionDetails, GetFunctionDetailsInput,
    GetFunctionDetailsOutput, ListFunctionsOutput, ListedFunction,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::health,
        routes::create_session,
        routes::close_session,
        routes::list_functions,
        routes::get_function_details,
        routes::execute_bash,
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
            // Execute bash
            ExecuteBashInput,
            ExecuteOutput,
            // Tool registration
            RegisterToolsRequest,
            CallbackConfig,
            RegisterToolsResponse,
            // Server registration
            RegisterMcpServersRequest,
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
        title = "pctx agent server API",
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
    allowed_origins: Vec<String>,
) -> Result<()> {
    let app = create_router(state, &allowed_origins);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("pctx agent server listening on http://{addr}");
    info!("OpenAPI documentation: http://{addr}/swagger-ui/");
    info!("");
    info!("Use REST API to register tools and MCP servers dynamically.");
    info!("WebSocket endpoint at ws://{addr}/ws for tool callbacks.",);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Create the Axum router with all routes
pub fn create_router<B: PctxSessionBackend>(
    state: AppState<B>,
    allowed_origins: &[String],
) -> Router {
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
        .route("/code-mode/execute-bash", post(routes::execute_bash))
        .route("/register/tools", post(routes::register_tools))
        .route("/register/servers", post(routes::register_servers))
        // WebSocket endpoint
        .route("/ws", get(websocket::ws_handler))
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Add state
        .with_state(state)
        // Add middleware - restrict CORS to explicitly allowed origins only
        .layer({
            let allowed = allowed_origins.to_owned();
            CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::predicate(
                    move |origin: &HeaderValue, _request_parts| {
                        let Ok(origin_str) = origin.to_str() else {
                            return false;
                        };

                        // Check if origin matches any allowed origin pattern
                        allowed.iter().any(|allowed_origin| {
                            // Support wildcard matching for ports (e.g., "http://localhost" matches any port)
                            if let (Ok(allowed_url), Ok(origin_url)) =
                                (url::Url::parse(allowed_origin), url::Url::parse(origin_str))
                            {
                                // Match scheme and host, ignore port if allowed origin has no explicit port
                                allowed_url.scheme() == origin_url.scheme()
                                    && allowed_url.host_str() == origin_url.host_str()
                                    && (allowed_url.port().is_none()
                                        || allowed_url.port() == origin_url.port())
                            } else {
                                // Fallback to exact string match
                                allowed_origin == origin_str
                            }
                        })
                    },
                ))
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any)
        })
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                // Extract trace context from headers using OpenTelemetry propagator
                let parent_cx = global::get_text_map_propagator(|propagator| {
                    propagator.extract(&HeaderExtractor(request.headers()))
                });

                // Check if we have a valid parent context
                let is_valid = parent_cx.span().span_context().is_valid();
                debug!(
                    traceparent = ?request.headers().get("traceparent"),
                    parent_valid = %is_valid,
                    "Extracting trace context"
                );

                // Create span with extracted context
                let span = tracing::info_span!(
                    "http_request",
                    method = %request.method(),
                    uri = %request.uri(),
                    version = ?request.version(),
                );

                // Set the parent OpenTelemetry context on the tracing span
                if is_valid {
                    if let Err(e) = span.set_parent(parent_cx) {
                        warn!(err = ?e, "Failed setting parent span context");
                    } else {
                        debug!("Successfully set parent span context");
                    }
                }

                span
            },
        ))
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
