use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use futures::Stream;
use opentelemetry::{global, trace::TraceContextExt};
use pctx_code_mode::registry::McpConnectionPool;
use rmcp::{
    ServiceExt,
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::{
        StreamableHttpServerConfig, stdio,
        streamable_http_server::{
            StreamableHttpService,
            session::{
                ServerSseMessage, SessionId, SessionManager,
                local::{LocalSessionManager, LocalSessionManagerError},
            },
        },
    },
};
use tabled::{
    Table,
    builder::Builder,
    settings::{
        Alignment, Color, Panel, Style, Width,
        object::{Cell, Columns, Rows},
        peaker::Priority,
        width::MinWidth,
    },
};
use terminal_size::terminal_size;
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{debug, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    extractors::HeaderExtractor,
    service::PctxMcpService,
    utils::{
        LOGO,
        styles::{fmt_cyan, fmt_dimmed},
    },
};

/// Wraps [`LocalSessionManager`] to cancel cached MCP connection pools when
/// a session is closed (HTTP DELETE or server-initiated close).
struct PctxSessionManager {
    inner: LocalSessionManager,
    pool_cache: Arc<Mutex<HashMap<String, Arc<McpConnectionPool>>>>,
}

impl PctxSessionManager {
    fn new(pool_cache: Arc<Mutex<HashMap<String, Arc<McpConnectionPool>>>>) -> Self {
        Self {
            inner: LocalSessionManager::default(),
            pool_cache,
        }
    }
}

impl SessionManager for PctxSessionManager {
    type Error = LocalSessionManagerError;
    type Transport = <LocalSessionManager as SessionManager>::Transport;

    async fn create_session(&self) -> Result<(SessionId, Self::Transport), Self::Error> {
        self.inner.create_session().await
    }

    async fn initialize_session(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<ServerJsonRpcMessage, Self::Error> {
        self.inner.initialize_session(id, message).await
    }

    async fn close_session(&self, id: &SessionId) -> Result<(), Self::Error> {
        let pool = self
            .pool_cache
            .lock()
            .ok()
            .and_then(|mut cache| cache.remove(id.as_ref()));
        if let Some(pool) = pool {
            debug!(session_id=%id, "Removed MCP connection pool cache for session");
            pool.cancel_all().await;
        }
        self.inner.close_session(id).await
    }

    async fn has_session(&self, id: &SessionId) -> Result<bool, Self::Error> {
        self.inner.has_session(id).await
    }

    async fn create_stream(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error> {
        self.inner.create_stream(id, message).await
    }

    async fn accept_message(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<(), Self::Error> {
        self.inner.accept_message(id, message).await
    }

    async fn create_standalone_stream(
        &self,
        id: &SessionId,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error> {
        self.inner.create_standalone_stream(id).await
    }

    async fn resume(
        &self,
        id: &SessionId,
        last_event_id: String,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error> {
        self.inner.resume(id, last_event_id).await
    }
}

pub struct PctxMcpServer {
    service: PctxMcpService,
    banner: bool,

    // HTTP transport configs
    host: String,
    port: u16,
    stateful_mode: bool,

    // stdio transport configs
    stdio_mode: bool,
}

impl PctxMcpServer {
    pub fn new(cfg: &pctx_code_mode::config::Config, code_mode: pctx_code_mode::CodeMode) -> Self {
        Self {
            service: PctxMcpService::new(&cfg, code_mode),
            banner: true,
            host: "127.0.0.1".into(),
            port: 8080,
            stateful_mode: false,
            stdio_mode: false,
        }
    }

    pub fn with_banner(mut self, banner: bool) -> Self {
        self.banner = banner;
        self
    }

    pub fn with_http_host(mut self, host: &str) -> Self {
        self.host = host.into();
        self
    }

    pub fn with_http_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_http_stateful(mut self, stateful: bool) -> Self {
        self.stateful_mode = stateful;
        self
    }

    pub fn with_stdio(mut self, stdio: bool) -> Self {
        let global_session = if stdio {
            Some(uuid::Uuid::new_v4())
        } else {
            None
        };
        self.service = self.service.with_global_session_id(global_session);
        self.stdio_mode = stdio;
        self
    }

    /// Serves MCP server with default Ctr + C shutdown signal
    ///
    /// # Panics
    ///
    /// Panics if the graceful shutdown with Ctr + C fails
    ///
    /// # Errors
    ///
    /// Errors if there is a failure starting the server on the configured host/port
    pub async fn serve(&self) -> Result<()> {
        let shutdown_signal = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed graceful shutdown");
        };
        if self.stdio_mode {
            self.serve_stdio_with_shutdown(shutdown_signal).await
        } else {
            self.serve_http_with_shutdown(shutdown_signal).await
        }
    }

    /// Serves MCP server with provided config, and shutdown signal
    ///
    ///
    /// # Errors
    ///
    /// Errors if there is a failure starting the server on the configured host/port
    pub async fn serve_http_with_shutdown<F>(&self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.banner();

        let mcp_service = self.service.clone();
        let session_manager = Arc::new(PctxSessionManager::new(self.service.pool_cache.clone()));

        let service = StreamableHttpService::new(
            move || Ok(mcp_service.clone()),
            session_manager,
            StreamableHttpServerConfig {
                stateful_mode: self.stateful_mode,
                ..Default::default()
            },
        );

        let router = axum::Router::new().nest_service("/mcp", service).layer(
            ServiceBuilder::new()
                // Generate UUID if x-request-id header doesn't exist
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                // Propagate x-request-id to response headers
                .layer(PropagateRequestIdLayer::x_request_id())
                // Add tracing layer that includes request_id in spans
                .layer(TraceLayer::new_for_http().make_span_with(
                    |request: &axum::http::Request<_>| {
                        let request_id = request
                            .extensions()
                            .get::<RequestId>()
                            .map_or("unknown".to_string(), |id| {
                                id.header_value().to_str().unwrap_or("invalid").to_string()
                            });

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
                        let span = tracing::error_span!(
                            "request",
                            method = %request.method(),
                            uri = %request.uri(),
                            version = ?request.version(),
                            request_id = %request_id,
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
                )),
        );
        let tcp_listener =
            tokio::net::TcpListener::bind(format!("{}:{}", &self.host, self.port)).await?;

        let _ = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(shutdown_signal)
            .await;

        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the stdio server fails to start or if the server
    /// task returns an error.
    pub async fn serve_stdio_with_shutdown<F>(&self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.banner();

        let mcp_service = self.service.clone();
        let mut shutdown_signal = Box::pin(shutdown_signal);
        let mut serve_task = tokio::spawn(mcp_service.serve(stdio()));
        let running = tokio::select! {
            () = &mut shutdown_signal => {
                serve_task.abort();
                return Ok(());
            }
            res = &mut serve_task => {
                res.map_err(|e| anyhow::anyhow!(e))?
                    .map_err(|e| anyhow::anyhow!("{e}"))?
            }
        };
        let cancel_token = running.cancellation_token();
        let mut join_handle = tokio::spawn(async move { running.waiting().await });

        tokio::select! {
            () = shutdown_signal => {
                cancel_token.cancel();
                let _ = join_handle.await;
            }
            res = &mut join_handle => {
                match res {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => return Err(anyhow::anyhow!(e)),
                    Err(e) => return Err(anyhow::anyhow!(e)),
                }
            }
        }

        Ok(())
    }

    fn banner_content(&self) -> Option<String> {
        if !self.banner {
            return None;
        }

        let logo_max_length = LOGO
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let min_term_width = logo_max_length + 4; // account for padding
        let term_width = terminal_size().map(|(w, _)| w.0).unwrap_or_default() as usize;

        if term_width < min_term_width {
            return None;
        }

        let mut builder = Builder::default();
        builder.push_record(["Server Name", &self.service.name]);
        builder.push_record(["Server Version", &self.service.version]);

        let http_transport_info = format!("http (stateful={})", self.stateful_mode);
        builder.push_record([
            "Transport",
            if self.stdio_mode {
                "stdio"
            } else {
                &http_transport_info
            },
        ]);
        if !self.stdio_mode {
            let mcp_url = format!("http://{}:{}/mcp", self.host, self.port);
            builder.push_record(["MCP URL", &mcp_url]);
        }
        builder.push_record(["Tool Disclosure", &self.service.disclosure.to_string()]);

        let active_tools = self
            .service
            .list_filtered_tools()
            .tools
            .iter()
            .map(|t| t.name.to_string())
            .collect::<Vec<String>>();
        builder.push_record(["Tools", &active_tools.join(", ")]);

        builder.push_record(["Docs", &fmt_dimmed("https://github.com/portofcontext/pctx")]);

        if !self.service.code_mode.tool_sets().is_empty() {
            builder.push_record(["", ""]);

            let tool_record = |s: &pctx_code_mode::codegen::ToolSet| {
                format!(
                    "{} - {} tool{}",
                    fmt_cyan(s.name.as_deref().unwrap_or_default()),
                    s.tools.len(),
                    if s.tools.len() > 1 { "s" } else { "" }
                )
            };
            builder.push_record([
                "Upstream MCPs",
                &self
                    .service
                    .code_mode
                    .tool_sets()
                    .first()
                    .map(tool_record)
                    .unwrap_or_default(),
            ]);
            for s in &self.service.code_mode.tool_sets()[1..] {
                builder.push_record(["", &tool_record(s)]);
            }
        }

        let table_width = term_width.min(80);
        let mut info_table = builder.build();
        info_table
            .with(Style::empty())
            .modify(Columns::first(), Color::BOLD)
            .modify(Columns::first(), MinWidth::new(20))
            .modify(Columns::new(..2), Width::wrap((term_width - 6) / 2));
        if !self.stdio_mode {
            info_table.modify(Cell::new(3, 1), Color::FG_BRIGHT_CYAN);
        }

        let logo_panel = Panel::header(format!("\n{LOGO}\n\n"));
        let logo_row = 0;
        let version_panel = Panel::header(format!(
            "pctx v{}\n\n",
            option_env!("CARGO_PKG_VERSION").unwrap_or_default()
        ));
        let version_row = 1;

        let style = Style::rounded().remove_horizontals().remove_vertical();
        let banner = Table::from_iter([[info_table.to_string()]])
            .with(style)
            .with(version_panel)
            .with(logo_panel)
            .with(Alignment::center())
            .modify(Rows::single(logo_row), Color::FG_BLUE)
            .modify(Rows::single(version_row), Color::FG_BLUE | Color::BOLD)
            .with((
                Width::wrap(table_width).priority(Priority::max(true)),
                Width::increase(table_width).priority(Priority::min(true)),
            ))
            .to_string();

        Some(format!("\n{banner}\n"))
    }

    fn banner(&self) {
        if let Some(content) = self.banner_content() {
            if self.stdio_mode {
                eprintln!("{content}");
            } else {
                println!("{content}"); // tracing::info doesn't work well with colors / formatting
            }
        }

        if self.stdio_mode {
            info!("PCTX listening via stdio...");
        } else {
            let mcp_url = format!(
                "http://{}:{}/mcp (stateful={})",
                self.host, self.port, self.stateful_mode
            );

            info!("PCTX listening at {mcp_url}...");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PctxMcpServer;
    use pctx_code_mode::config::Config;

    #[tokio::test]
    async fn test_serve_stdio_with_immediate_shutdown() {
        let cfg = Config::default();
        let code_mode = pctx_code_mode::CodeMode::default();
        let server = PctxMcpServer::new(&cfg, code_mode).with_stdio(true);

        let result = server.serve_stdio_with_shutdown(async {}).await;

        assert!(
            result.is_ok(),
            "unexpected stdio shutdown error: {}",
            result.err().unwrap()
        );
    }
}
