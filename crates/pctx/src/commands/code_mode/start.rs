use anyhow::Result;
use clap::Parser;
use pctx_code_mode::CodeMode;
use pctx_config::Config;
use tracing::info;

use pctx_mcp_server::PctxMcpServer;

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

    /// Serve MCP over stdio instead of HTTP
    #[arg(long)]
    pub stdio: bool,

    /// Use stateful MCP sessions (incompatible with --stdio)
    #[arg(long, conflicts_with = "stdio")]
    pub stateful_http: bool,
}

impl StartCmd {
    pub(crate) async fn load_code_mode(cfg: &Config) -> Result<CodeMode> {
        // Connect to each MCP server and fetch their tool definitions in parallel
        info!(
            "Creating code mode interface for {} upstream MCP servers (parallel)",
            cfg.servers.len()
        );
        let code_mode = CodeMode::default().with_servers(&cfg.servers, 30).await?;

        info!(
            "Code mode initialized with {} upstream MCP servers",
            cfg.servers.len()
        );

        Ok(code_mode)
    }

    pub(crate) async fn handle(&self, cfg: Config) -> Result<Config> {
        if cfg.servers.is_empty() {
            anyhow::bail!(
                "No upstream MCP servers configured. Add servers with 'pctx add <name> <url>'"
            );
        }

        let code_mode = StartCmd::load_code_mode(&cfg).await?;

        let pctx_mcp = PctxMcpServer::new(&cfg, code_mode)
            .with_banner(!self.no_banner)
            .with_http_host(&self.host)
            .with_http_port(self.port)
            .with_http_stateful(self.stateful_http)
            .with_stdio(self.stdio);

        pctx_mcp.serve().await?;

        info!("Shutting down...");

        Ok(cfg)
    }
}
