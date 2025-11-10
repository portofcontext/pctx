pub(crate) mod client;
pub(crate) mod tools;
pub(crate) mod upstream;

use anyhow::Result;
use console::{Alignment, Term, measure_text_width, pad_str};
use log::info;
use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
};
use textwrap::wrap;

use crate::utils::{
    LOGO,
    styles::{fmt_bold, fmt_green},
};
use crate::{
    mcp::tools::{PtcxTools, UpstreamMcp},
    utils::styles::fmt_cyan,
};

pub(crate) struct PctxMcp;
impl PctxMcp {
    pub(crate) async fn serve(host: &str, port: u16, upstream: Vec<UpstreamMcp>) -> Result<()> {
        let allowed_hosts = upstream
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

        Self::log_banner(host, port, &upstream);

        let service = StreamableHttpService::new(
            move || Ok(PtcxTools::new(allowed_hosts.clone()).with_upstream_mcps(upstream.clone())),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig {
                stateful_mode: false,
                ..Default::default()
            },
        );

        let router = axum::Router::new().nest_service("/mcp", service);
        let tcp_listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;

        let _ = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed graceful shutdown");
            })
            .await;

        Ok(())
    }

    fn log_banner(host: &str, port: u16, upstream: &[UpstreamMcp]) {
        let term = Term::stdout();
        if term.is_term() {
            let (_, term_width) = term.size();
            let width = (term_width as usize).min(80);
            // Calculate minimum width needed for logo
            let logo_width = LOGO.lines().map(measure_text_width).max().unwrap_or(0);
            let min_width = logo_width + 4; // +2 for borders, +2 for padding

            if width > min_width {
                let border = "─".repeat(width.saturating_sub(2));

                info!("\n╭{border}╮");

                // Center the logo
                for line in LOGO.lines() {
                    let colored_line = fmt_cyan(line);
                    info!(
                        "│{}│",
                        pad_str(
                            &colored_line,
                            width.saturating_sub(2),
                            Alignment::Center,
                            None
                        )
                    );
                }

                // Content lines
                let mut lines = vec![
                    String::new(),
                    format!("Listening at http://{host}:{port}/mcp..."),
                    format!(
                        "{}: {}",
                        fmt_bold("Tools"),
                        [
                            fmt_green("list_functions"),
                            fmt_green("get_function_details"),
                            fmt_green("execute"),
                        ]
                        .join(", ")
                    ),
                    String::new(),
                ];

                if !upstream.is_empty() {
                    lines.push(format!("Upstream servers: {}", upstream.len()));
                    for u in upstream {
                        lines.push(format!(
                            "  • {url} ({num_tools} tool{plural})",
                            url = u.url,
                            num_tools = u.tools.len(),
                            plural = if u.tools.len() > 1 { "s" } else { "" }
                        ));
                    }
                    lines.push(String::new());
                }

                for line in lines {
                    let visual_width = measure_text_width(&line);
                    if visual_width > width.saturating_sub(2) {
                        // Wrap long lines
                        let wrapped = wrap(&line, width.saturating_sub(2));
                        for wrapped_line in wrapped {
                            info!(
                                "│{}│",
                                pad_str(
                                    &wrapped_line,
                                    width.saturating_sub(2),
                                    Alignment::Center,
                                    None
                                )
                            );
                        }
                    } else {
                        info!(
                            "│{}│",
                            pad_str(&line, width.saturating_sub(2), Alignment::Center, None)
                        );
                    }
                }

                info!("╰{border}╯\n");

                return;
            }
        }

        info!("PCTX");
        info!("Listening at http://{host}:{port}/mcp...");
    }
}
