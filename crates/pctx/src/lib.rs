pub mod commands;
pub mod utils;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use serde_json::json;
use std::io::{self, Write};

use crate::utils::{logger::init_cli_logger, telemetry::init_telemetry};
use pctx_config::Config;

#[derive(Parser)]
#[command(name = "pctx")]
#[command(version)]
#[command(about = "PCTX - Code Mode")]
#[command(
    long_about = "Use PCTX to expose code mode either as a session based server or by aggregating multiple MCP servers into a single code mode MCP server."
)]
#[command(after_help = "EXAMPLES:\n  \
    # Code Mode sessions\n  \
    pctx start\n  \
    # Code Mode MCP\n  \
    pctx mcp init \n  \
    pctx mcp add my-server https://mcp.example.com\n  \
    pctx mcp dev\n\n  \
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Config file path, defaults to ./pctx.json
    #[arg(long, short = 'c', global = true, default_value_t = Config::default_path())]
    pub config: Utf8PathBuf,

    /// No logging except for errors
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Verbose logging (-v) or trace logging (-vv)
    #[arg(long, short = 'v', action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

impl Cli {
    fn cli_logger(&self) -> bool {
        !matches!(
            &self.command,
            Commands::Mcp(McpCommands::Start(_) | McpCommands::Dev(_))
        )
    }

    fn json_l(&self) -> Option<Utf8PathBuf> {
        if let Commands::Mcp(McpCommands::Dev(dev)) = &self.command {
            Some(dev.log_file.clone())
        } else {
            None
        }
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn handle(&self) -> anyhow::Result<()> {
        match &self.command {
            Commands::Mcp(mcp_cmd) => self.handle_mcp(mcp_cmd).await,
            Commands::Start(start_cmd) => {
                let cfg = Config::load(&self.config).unwrap_or_default();
                // Session server uses stdout for logs (not stdio protocol)
                init_telemetry(&cfg, None, false).await?;

                start_cmd.handle().await
            }
        }
    }

    async fn handle_mcp(&self, cmd: &McpCommands) -> anyhow::Result<()> {
        let cfg = Config::load(&self.config);

        if let (McpCommands::Start(start_cmd), Err(err)) = (cmd, &cfg)
            && start_cmd.stdio
        {
            return Self::handle_stdio_config_error(err);
        }

        if self.cli_logger() {
            init_cli_logger(self.verbose, self.quiet);
        } else if let Ok(c) = &cfg {
            // Use stderr for stdio mode to keep stdout clean for JSON-RPC
            let use_stderr = matches!(cmd, McpCommands::Start(start_cmd) if start_cmd.stdio);
            init_telemetry(c, self.json_l(), use_stderr).await?;
        }

        let _updated_cfg = match cmd {
            McpCommands::Init(cmd) => cmd.handle(&self.config).await?,
            McpCommands::List(cmd) => cmd.handle(cfg?).await?,
            McpCommands::Add(cmd) => cmd.handle(cfg?, true).await?,
            McpCommands::Remove(cmd) => cmd.handle(cfg?)?,
            McpCommands::Start(cmd) => cmd.handle(cfg?).await?,
            McpCommands::Dev(cmd) => cmd.handle(cfg?).await?,
        };

        Ok(())
    }

    fn handle_stdio_config_error(err: &anyhow::Error) -> anyhow::Result<()> {
        let response = build_stdio_error_response(err.to_string().as_str());
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{response}")?;
        stdout.flush()?;

        // Intentionally return the error so stderr includes a human-readable message.
        Err(anyhow::anyhow!(err.to_string()))
    }
}

fn build_stdio_error_response(message: &str) -> String {
    let response = json!({
        "jsonrpc": "2.0",
        "id": serde_json::Value::Null,
        "error": {
            "code": STDIO_CONFIG_ERROR_CODE,
            "message": message,
        }
    });

    response.to_string()
}

const STDIO_CONFIG_ERROR_CODE: i32 = -32000;

#[cfg(test)]
mod tests {
    use super::build_stdio_error_response;

    #[test]
    fn stdio_error_response_defaults_id_to_null() {
        let response = build_stdio_error_response("missing config");

        assert!(response.contains(r#""id":null"#));
    }
}

#[derive(Debug, Subcommand)]
#[command(styles=utils::styles::get_styles())]
pub enum Commands {
    /// Start PCTX server for code mode sessions
    #[command(
        long_about = "Starts PCTX server with no pre-configured tools. Use a client library like `pip install pctx-client` to create sessions, register tools, and expose code-mode tools to agent libraries."
    )]
    Start(commands::start::StartCmd),

    /// MCP server commands (with pctx.json configuration)
    #[command(subcommand)]
    Mcp(McpCommands),
}

#[derive(Debug, Subcommand)]
pub enum McpCommands {
    /// Initialize pctx.json configuration file
    #[command(long_about = "Initialize pctx.json configuration file.")]
    Init(commands::mcp::InitCmd),

    /// List MCP servers and test connections
    #[command(long_about = "Lists configured MCP servers and tests the connection to each.")]
    List(commands::mcp::ListCmd),

    /// Add an MCP server to configuration (HTTP or stdio)
    #[command(
        long_about = "Add a new MCP server to the configuration. Supports both HTTP(S) URLs and stdio-based servers via the --command flag."
    )]
    Add(commands::mcp::AddCmd),

    /// Remove an MCP server from configuration
    #[command(long_about = "Remove an MCP server from the configuration.")]
    Remove(commands::mcp::RemoveCmd),

    /// Start the PCTX MCP server
    #[command(long_about = "Start the PCTX MCP server (exposes /mcp endpoint).")]
    Start(commands::mcp::StartCmd),

    /// Start the PCTX MCP server with terminal UI
    #[command(
        long_about = "Start the PCTX MCP server in development mode with an interactive terminal UI with data and logging."
    )]
    Dev(commands::mcp::DevCmd),
}
