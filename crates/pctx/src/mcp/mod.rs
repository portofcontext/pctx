// pub(crate) mod client;
pub(crate) mod tools;
// pub(crate) mod upstream; // Moved to pctx_lib

use anyhow::Result;
use pctx_config::Config;
use pctx_lib::UpstreamMcp;
use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
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
use tracing::info;

use crate::utils::{LOGO, styles::fmt_dimmed};
use crate::{mcp::tools::PtcxTools, utils::styles::fmt_cyan};

pub(crate) struct PctxMcp {
    config: Config,
    upstream: Vec<UpstreamMcp>,
    host: String,
    port: u16,
    banner: bool,
}

impl PctxMcp {
    pub(crate) fn new(
        config: Config,
        upstream: Vec<UpstreamMcp>,
        host: &str,
        port: u16,
        banner: bool,
    ) -> Self {
        Self {
            config,
            upstream,
            host: host.into(),
            port,
            banner,
        }
    }

    pub(crate) async fn serve(&self) -> Result<()> {
        self.serve_with_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed graceful shutdown");
        })
        .await
    }

    pub(crate) async fn serve_with_shutdown<F>(&self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let allowed_hosts = self
            .upstream
            .iter()
            .filter_map(|m| {
                let host = m.url.host_str()?;
                if let Some(port) = m.url.port() {
                    Some(format!("{host}:{port}"))
                } else {
                    let default_port = if m.url.scheme() == "https" { 443 } else { 80 };
                    Some(format!("{host}:{default_port}"))
                }
            })
            .collect::<Vec<_>>();

        self.banner();

        let tools = PtcxTools::new(self.config.clone(), allowed_hosts.clone())
            .with_upstream_mcps(self.upstream.clone());
        let service = StreamableHttpService::new(
            move || Ok(tools.clone()),
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

                        tracing::error_span!(
                            "request",
                            method = %request.method(),
                            uri = %request.uri(),
                            version = ?request.version(),
                            request_id = %request_id,
                        )
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

    fn banner(&self) {
        let mcp_url = format!("http://{}:{}/mcp", self.host, self.port);
        let logo_max_length = LOGO
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let min_term_width = logo_max_length + 4; // account for padding
        let term_width = terminal_size().map(|(w, _)| w.0).unwrap_or_default() as usize;

        if self.banner && term_width >= min_term_width {
            let mut builder = Builder::default();

            builder.push_record(["Server Name", &self.config.name]);
            builder.push_record(["Server Version", &self.config.version]);
            builder.push_record(["Server URL", &mcp_url]);
            builder.push_record([
                "Tools",
                &["list_functions", "get_function_details", "execute"].join(", "),
            ]);
            builder.push_record(["Docs", &fmt_dimmed("https://github.com/portofcontext/pctx")]);

            if !self.upstream.is_empty() {
                builder.push_record(["", ""]);

                let tool_record = |u: &UpstreamMcp| {
                    format!(
                        "{} - {} tool{}",
                        fmt_cyan(&u.name),
                        u.tools.len(),
                        if u.tools.len() > 1 { "s" } else { "" }
                    )
                };
                builder.push_record([
                    "Upstream MCPs",
                    &self.upstream.first().map(tool_record).unwrap_or_default(),
                ]);
                for u in &self.upstream[1..] {
                    builder.push_record(["", &tool_record(u)]);
                }
            }

            let table_width = (term_width).min(80) as usize;
            let info_table = builder
                .build()
                .with(Style::empty())
                .modify(Columns::first(), Color::BOLD)
                .modify(Cell::new(2, 1), Color::FG_CYAN)
                .modify(Columns::first(), MinWidth::new(20))
                .modify(Columns::new(..2), Width::wrap((term_width - 6) / 2)) // info cols should have equal space
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

            println!("\n{banner}\n"); // tracing::info doesn't work well with colors / formatting
        }

        info!("PCTX listening at {mcp_url}...");
    }
}
