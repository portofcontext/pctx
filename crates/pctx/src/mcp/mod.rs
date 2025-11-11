// pub(crate) mod client;
pub(crate) mod tools;
pub(crate) mod upstream;

use anyhow::Result;
use log::info;
use pctx_config::Config;
use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
};
use tabled::{
    builder::Builder,
    settings::{
        Alignment, Color, Panel, Style, Width,
        object::{Cell, Columns, Object, Rows},
    },
};
use terminal_size::terminal_size;

use crate::mcp::{tools::PtcxTools, upstream::UpstreamMcp};
use crate::utils::LOGO;

pub(crate) struct PctxMcp {
    config: Config,
    upstream: Vec<UpstreamMcp>,
    host: String,
    port: u16,
}

impl PctxMcp {
    pub(crate) fn new(config: Config, upstream: Vec<UpstreamMcp>, host: &str, port: u16) -> Self {
        Self {
            config,
            upstream,
            host: host.into(),
            port,
        }
    }

    pub(crate) async fn serve(&self) -> Result<()> {
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

        let tools = PtcxTools::new(allowed_hosts.clone()).with_upstream_mcps(self.upstream.clone());
        let service = StreamableHttpService::new(
            move || Ok(tools.clone()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig {
                stateful_mode: false,
                ..Default::default()
            },
        );

        let router = axum::Router::new().nest_service("/mcp", service);
        let tcp_listener =
            tokio::net::TcpListener::bind(format!("{}:{}", &self.host, self.port)).await?;

        let _ = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed graceful shutdown");
            })
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

        if term_width >= min_term_width {
            let mut builder = Builder::default();

            builder.push_record(["🦀 Server Name", &self.config.name]);
            builder.push_record(["🌎 Server URL", &mcp_url]);
            builder.push_record([
                "🔨 Tools",
                &["list_functions", "get_function_details", "execute"].join(", "),
            ]);
            builder.push_record(["📖 Docs", "https://github.com/portofcontext/pctx"]);

            if !self.upstream.is_empty() {
                builder.push_record(["", ""]);

                let tool_record = |u: &UpstreamMcp| {
                    format!(
                        "{} - {} tool{}",
                        &u.name,
                        u.tools.len(),
                        if u.tools.len() > 1 { "s" } else { "" }
                    )
                };
                builder.push_record([
                    "🤖 Upstream MCPs",
                    &self.upstream.first().map(tool_record).unwrap_or_default(),
                ]);
                for u in &self.upstream[1..] {
                    builder.push_record(["", &tool_record(u)]);
                }
            }

            let logo_panel = Panel::header(format!("\n{LOGO}\n\n"));
            let logo_row = 0;
            let version_panel = Panel::header(format!(
                "v{}\n\n",
                option_env!("CARGO_PKG_VERSION").unwrap_or_default()
            ));
            let version_row = 1;

            let info_start_row = 2;
            let info_title_col = 0;
            let info_val_col = 1;

            let style = Style::rounded().remove_horizontals().remove_vertical();
            let table_width = term_width.min(120) as usize;
            println!("{table_width}");
            let banner = builder
                .build()
                .with(Width::truncate(table_width))
                .with(style)
                .with(version_panel)
                .with(logo_panel)
                // style and align the logo and version
                .modify(Rows::single(logo_row), Color::FG_CYAN)
                .modify(
                    Rows::single(version_row),
                    Color::FG_BRIGHT_BLUE | Color::BOLD,
                )
                .modify(Rows::new(logo_row..=version_row), Alignment::center())
                // style info rows & cols
                .modify(
                    Rows::new(info_start_row..),
                    Width::wrap(table_width / 2).keep_words(true),
                ) // info cols should have equal space
                .modify(
                    Rows::new(info_start_row..).intersect(Columns::single(info_title_col)),
                    Color::BOLD,
                )
                .modify(Cell::new(info_start_row + 2, info_val_col), Color::FG_GREEN)
                .to_string();

            info!("\n{banner}\n");
        } else {
            info!("PCTX listening at {mcp_url}...");
        }
    }
}
