pub mod commands;
pub mod utils;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use serde_json::json;
use std::io::{self, Write};

use crate::utils::{
    logger::{self, init_cli_logger},
    telemetry::init_telemetry,
};
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
    /// `-v` and `-q` are global, so every command resolves its logging here.
    async fn init_logging(&self, cfg: &Config) -> anyhow::Result<()> {
        let level = logger::flag_level(self.verbose, self.quiet);

        match &self.command {
            // Short-lived commands print for a human, the rest emit structured logs
            Commands::Mcp(
                McpCommands::Init(_)
                | McpCommands::List(_)
                | McpCommands::Add(_)
                | McpCommands::Remove(_),
            ) => {
                init_cli_logger(self.verbose, self.quiet);
                Ok(())
            }
            // Dev writes JSONL for its TUI to tail
            Commands::Mcp(McpCommands::Dev(dev)) => {
                init_telemetry(cfg, Some(dev.log_file.clone()), false, level).await
            }
            // Stdio mode keeps stdout clean for JSON-RPC
            Commands::Mcp(McpCommands::Start(start_cmd)) => {
                init_telemetry(cfg, None, start_cmd.stdio, level).await
            }
            Commands::Start(_) => init_telemetry(cfg, None, false, level).await,
        }
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn handle(&self) -> anyhow::Result<()> {
        let cfg = Config::load(&self.config);

        if let (Commands::Mcp(McpCommands::Start(start_cmd)), Err(err)) = (&self.command, &cfg)
            && start_cmd.stdio
        {
            return Self::handle_stdio_config_error(err);
        }

        // A broken config still gets logging, so the error is reported the usual way
        let fallback = Config::default();
        self.init_logging(cfg.as_ref().unwrap_or(&fallback)).await?;

        match &self.command {
            Commands::Start(start_cmd) => start_cmd.handle().await,
            Commands::Mcp(mcp_cmd) => self.handle_mcp(mcp_cmd, cfg).await,
        }
    }

    async fn handle_mcp(
        &self,
        cmd: &McpCommands,
        cfg: anyhow::Result<Config>,
    ) -> anyhow::Result<()> {
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
