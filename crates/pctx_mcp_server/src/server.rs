use anyhow::Result;
use opentelemetry::{global, trace::TraceContextExt};
use pctx_config::Config;
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpServerConfig, stdio,
        streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
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

pub struct PctxMcpServer {
    host: String,
    port: u16,
    banner: bool,
}

impl PctxMcpServer {
    pub fn new(host: &str, port: u16, banner: bool) -> Self {
        Self {
            host: host.into(),
            port,
            banner,
        }
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
    pub async fn serve(&self, cfg: &Config, code_mode: pctx_code_mode::CodeMode) -> Result<()> {
        let shutdown_signal = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed graceful shutdown");
        };
        self.serve_with_shutdown(cfg, code_mode, shutdown_signal)
            .await
    }

    /// Serves MCP server with provided config, and shutdown signal
    ///
    ///
    /// # Errors
    ///
    /// Errors if there is a failure starting the server on the configured host/port
    pub async fn serve_with_shutdown<F>(
        &self,
        cfg: &Config,
        code_mode: pctx_code_mode::CodeMode,
        shutdown_signal: F,
    ) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.banner_http(cfg, &code_mode);

        let mcp_service = PctxMcpService::new(cfg, code_mode);

        let service = StreamableHttpService::new(
            move || Ok(mcp_service.clone()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig {
                stateful_mode: false,
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
    /// Returns an error if the stdio server fails to start or shut down cleanly.
    ///
    /// # Panics
    ///
    /// Panics if the ctrl-c handler cannot be installed.
    pub async fn serve_stdio(
        &self,
        cfg: &Config,
        code_mode: pctx_code_mode::CodeMode,
    ) -> Result<()> {
        let shutdown_signal = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed graceful shutdown");
        };
        self.serve_stdio_with_shutdown(cfg, code_mode, shutdown_signal)
            .await
    }

    /// # Errors
    ///
    /// Returns an error if the stdio server fails to start or if the server
    /// task returns an error.
    pub async fn serve_stdio_with_shutdown<F>(
        &self,
        cfg: &Config,
        code_mode: pctx_code_mode::CodeMode,
        shutdown_signal: F,
    ) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.banner_stdio(cfg, &code_mode);

        let mcp_service = PctxMcpService::new(cfg, code_mode);
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

    fn banner(
        &self,
        cfg: &pctx_config::Config,
        code_mode: &pctx_code_mode::CodeMode,
        transport_label: &str,
        transport_value: &str,
    ) -> Option<String> {
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
        builder.push_record(["Server Name", &cfg.name]);
        builder.push_record(["Server Version", &cfg.version]);
        builder.push_record([transport_label, transport_value]);
        builder.push_record([
            "Tools",
            &["list_functions", "get_function_details", "execute"].join(", "),
        ]);
        builder.push_record(["Docs", &fmt_dimmed("https://github.com/portofcontext/pctx")]);

        if !code_mode.tool_sets().is_empty() {
            builder.push_record(["", ""]);

            let tool_record = |s: &pctx_codegen::ToolSet| {
                format!(
                    "{} - {} tool{}",
                    fmt_cyan(&s.name),
                    s.tools.len(),
                    if s.tools.len() > 1 { "s" } else { "" }
                )
            };
            builder.push_record([
                "Upstream MCPs",
                &code_mode
                    .tool_sets()
                    .first()
                    .map(tool_record)
                    .unwrap_or_default(),
            ]);
            for s in &code_mode.tool_sets()[1..] {
                builder.push_record(["", &tool_record(s)]);
            }
        }

        let table_width = term_width.min(80);
        let info_table = builder
            .build()
            .with(Style::empty())
            .modify(Columns::first(), Color::BOLD)
            .modify(Cell::new(2, 1), Color::FG_CYAN)
            .modify(Columns::first(), MinWidth::new(20))
            .modify(Columns::new(..2), Width::wrap((term_width - 6) / 2))
            .to_string();

        let logo_panel = Panel::header(format!("\n{LOGO}\n\n"));
        let logo_row = 0;
        let version_panel = Panel::header(format!(
            "pctx v{}\n\n",
            option_env!("CARGO_PKG_VERSION").unwrap_or_default()
        ));
        let version_row = 1;

        let style = Style::rounded().remove_horizontals().remove_vertical();
        let banner = Table::from_iter([[info_table]])
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

    fn banner_http(&self, cfg: &pctx_config::Config, code_mode: &pctx_code_mode::CodeMode) {
        let mcp_url = format!("http://{}:{}/mcp", self.host, self.port);

        if let Some(banner) = self.banner(cfg, code_mode, "Server URL", &mcp_url) {
            println!("{banner}"); // tracing::info doesn't work well with colors / formatting
        }

        info!("PCTX listening at {mcp_url}...");
    }

    fn banner_stdio(&self, cfg: &pctx_config::Config, code_mode: &pctx_code_mode::CodeMode) {
        if let Some(banner) = self.banner(cfg, code_mode, "Transport", "stdio") {
            eprintln!("{banner}");
        }

        info!("PCTX listening via stdio...");
    }
}

#[cfg(test)]
mod tests {
    use super::PctxMcpServer;
    use pctx_config::Config;

    #[tokio::test]
    async fn test_serve_stdio_with_immediate_shutdown() {
        let server = PctxMcpServer::new("127.0.0.1", 0, false);
        let cfg = Config::default();
        let code_mode = pctx_code_mode::CodeMode::default();

        let result = server
            .serve_stdio_with_shutdown(&cfg, code_mode, async {})
            .await;

        assert!(
            result.is_ok(),
            "unexpected stdio shutdown error: {}",
            result.err().unwrap()
        );
    }

    // Note: test_serve_stdio_with_delayed_shutdown removed because it's difficult to test
    // stdio transport without actual stdin. The immediate shutdown test above covers
    // the basic shutdown mechanism.

    #[test]
    fn test_server_construction() {
        let server = PctxMcpServer::new("127.0.0.1", 8080, true);
        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 8080);
        assert!(server.banner);

        let server = PctxMcpServer::new("0.0.0.0", 3000, false);
        assert_eq!(server.host, "0.0.0.0");
        assert_eq!(server.port, 3000);
        assert!(!server.banner);
    }
}
