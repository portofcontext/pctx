use anyhow::Result;
use clap::Parser;
use pctx_code_mode::{CodeMode, ExecutorPool, PoolConfig};
use pctx_config::Config;
use std::sync::Arc;
use tracing::{info, warn};

use pctx_mcp_server::PctxMcpServer;

#[derive(Debug, Clone, Parser)]
pub struct StartCmd {
    /// Port to listen on
    #[arg(short, long, default_value = "8080", env = "PCTX_PORT")]
    pub port: u16,

    /// Host address to bind to (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1", env = "PCTX_HOST")]
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

    /// Number of worker processes in the executor pool.
    /// Defaults to the number of logical CPUs, capped at 8.
    /// Set to 0 to disable the pool and run in-process.
    #[arg(long, env = "PCTX_WORKERS")]
    pub workers: Option<usize>,
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

        let worker_count = self.workers.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().min(8))
                .unwrap_or(4)
        });

        let code_mode = StartCmd::load_code_mode(&cfg).await?;
        let code_mode = if worker_count == 0 {
            info!("Executor pool disabled (--workers 0), running in-process");
            code_mode
        } else {
            match PoolConfig::from_current_exe(worker_count) {
                Ok(pool_cfg) => match ExecutorPool::new(pool_cfg).await {
                    Ok(pool) => {
                        info!("Executor pool ready ({worker_count} workers)");
                        code_mode.with_executor_pool(Arc::new(pool))
                    }
                    Err(e) => {
                        warn!("Failed to start executor pool, falling back to in-process execution: {e}");
                        code_mode
                    }
                },
                Err(e) => {
                    warn!("Could not locate worker binary, falling back to in-process execution: {e}");
                    code_mode
                }
            }
        };

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
