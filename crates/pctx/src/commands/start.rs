use anyhow::Result;
use clap::Parser;
use pctx_config::Config;
use tracing::{debug, info, warn};

use crate::mcp::{PctxMcp, upstream::UpstreamMcp};

#[derive(Debug, Clone, Parser)]
pub struct StartCmd {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host address to bind to (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Don't show the server banner
    #[arg(long)]
    pub no_banner: bool,
}

impl StartCmd {
    pub(crate) async fn handle(&self, cfg: Config) -> Result<Config> {
        if cfg.servers.is_empty() {
            anyhow::bail!(
                "No upstream MCP servers configured. Add servers with 'pctx add <name> <url>'"
            );
        }

        // Connect to each MCP server and fetch their tool definitions
        info!(
            "Creating code mode interface for {} upstream MCP servers",
            cfg.servers.len()
        );
        let mut upstream_servers = Vec::new();
        for server in &cfg.servers {
            debug!("Creating code mode interface for {}", &server.name);
            match UpstreamMcp::from_server(server).await {
                Ok(upstream) => {
                    upstream_servers.push(upstream);
                }
                Err(e) => {
                    warn!(
                        err =? e,
                        server.name =? &server.name,
                        server.url =? server.url.to_string(),
                        "Failed creating creating code mode for `{}` MCP server",
                        &server.name
                    );
                }
            }
        }

        PctxMcp::new(
            cfg.clone(),
            upstream_servers,
            &self.host,
            self.port,
            !self.no_banner,
        )
        .serve()
        .await?;

        info!("Shutting down...");

        Ok(cfg)
    }
}
