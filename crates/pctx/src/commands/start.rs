use anyhow::Result;
use clap::Parser;
use log::{info, warn};
use pctx_config::Config;

use crate::{
    mcp::{PctxMcp, upstream::UpstreamMcp},
    utils::{
        CHECK, MARK,
        spinner::Spinner,
        styles::{fmt_bold, fmt_cyan, fmt_error, fmt_green, fmt_red, fmt_yellow},
    },
};

#[derive(Debug, Clone, Parser)]
pub struct StartCmd {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host address to bind to (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}

impl StartCmd {
    pub(crate) async fn handle(&self, cfg: Config) -> Result<Config> {
        if cfg.servers.is_empty() {
            anyhow::bail!(
                "No upstream MCP servers configured. Add servers with 'pctx mcp add <name> <url>'"
            );
        }

        // Connect to each MCP server and fetch their tool definitions
        let mut sp = Spinner::new("");

        let mut upstream_servers = Vec::new();
        let mut fails = Vec::new();
        for server in &cfg.servers {
            sp.update_text(format!(
                "Creating {} interface for {}",
                fmt_bold("Code Mode"),
                fmt_cyan(&server.name)
            ));
            match UpstreamMcp::from_server(server).await {
                Ok(upstream) => {
                    upstream_servers.push(upstream);
                }
                Err(e) => {
                    fails.push(fmt_error(&format!(
                        "Failed creating {} for {}: {e}",
                        fmt_bold("Code Mode"),
                        fmt_cyan(&server.name)
                    )));
                }
            }
        }

        let symbol = if upstream_servers.len() == cfg.servers.len() {
            fmt_green(CHECK)
        } else if upstream_servers.is_empty() {
            fmt_red(MARK)
        } else {
            fmt_yellow("~")
        };

        sp.stop_and_persist(
            &symbol,
            format!(
                "{} interface generated for {} upstream MCP servers",
                fmt_bold("Code Mode"),
                fmt_cyan(&upstream_servers.len().to_string())
            ),
        );
        for fail in fails {
            warn!("{fail}");
        }

        // Start the gateway with multiple MCP servers
        PctxMcp::new(cfg.clone(), upstream_servers, &self.host, self.port)
            .serve()
            .await?;

        info!("Shutting down...");

        Ok(cfg)
    }
}
