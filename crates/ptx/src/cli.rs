#[cfg(all(
    not(target_env = "msvc"),
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64"
    )
))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(target_os = "windows")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod commands;
mod mcp;
mod utils;

use clap::{Parser, Subcommand};
use log::error;

#[derive(Parser)]
#[command(name = "pctx")]
#[command(version)]
#[command(about = "PCTX - Code Mode MCP Gateway")]
#[command(
    long_about = "PCTX is a code mode MCP (Model Context Protocol) gateway that aggregates multiple MCP servers \
into a single endpoint and presents them as a TypeScript API for AI agents to call via code execution.\n\n\
Unlike traditional MCP implementations where agents directly call tools, PTCX exposes tools as TypeScript functions. \
This allows agents to write code that calls MCP servers more efficiently, loading only the tools they need and \
processing data in the execution environment before passing results to the model.\n\n\
PTCX supports various authentication methods including OAuth 2.1, making it easy to connect AI assistants to \
protected MCP servers while keeping credentials secure."
)]
#[command(after_help = "EXAMPLES:\n  \
    # Initialize configuration\n  \
    pctx init\n\n  \
    # Add an MCP server with OAuth 2.1 authentication\n  \
    pctx mcp add my-server https://mcp.example.com --auth oauth2\n  \
    pctx mcp auth my-server\n\n  \
    # List servers and check their health\n  \
    pctx mcp list\n\n  \
    # Start the gateway\n  \
    pctx start --port 8080\n\
")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// No logging except for errors
    #[arg(long, short = 'q', global = true)]
    quiet: bool,

    /// Verbose logging (-v) or trace logging (-vv)
    #[arg(long, short = 'v', action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize PCTX configuration directory and files
    #[command(
        long_about = "Creates the ~/.pctl directory and initializes the configuration file. \
This command is safe to run multiple times - it will not overwrite existing configuration."
    )]
    Init,

    /// Start the MCP gateway server
    #[command(
        long_about = "Starts the PCTX gateway server that aggregates all configured MCP servers. \
The gateway exposes a single MCP endpoint at /mcp that provides access to tools from all configured servers.\n\n\
Before starting, ensure you have:\n\
  1. Initialized configuration with 'pctx init'\n\
  2. Added at least one MCP server with 'pctx mcp add'\n\
  3. Configured authentication if required with 'pctx mcp auth'"
    )]
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Host address to bind to (use 0.0.0.0 for external access)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Configure and manage MCP servers
    #[command(
        long_about = "Manage MCP server configurations including adding, removing, and testing servers. \
Each server can have its own authentication configuration supporting:\n\
  - OAuth 2.1 (recommended for HTTP servers)\n\
  - Environment variables\n\
  - System keychain\n\
  - External command execution"
    )]
    Mcp {
        #[command(subcommand)]
        mcp_cmd: McpCommands,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    /// Add a new MCP server to the configuration
    #[command(
        long_about = "Register a new MCP server with PCTX. After adding a server, you may need to configure \
authentication using 'pctx mcp auth <name>' if the server requires it.\n\n\
AUTHENTICATION TYPES:\n\
  oauth2   - OAuth 2.1 authorization (recommended for HTTP servers)\n\
  env      - Environment variable containing bearer token\n\
  keychain - System keychain storage\n\
  command  - External command that outputs token\n\n\
EXAMPLES:\n\
  # Add a server without authentication\n\
  pctx mcp add local http://localhost:3000/mcp\n\n\
  # Add a server and configure OAuth 2.1 later\n\
  pctx mcp add prod https://mcp.example.com\n\
  pctx mcp auth prod"
    )]
    Add {
        /// Unique name for this server
        name: String,

        /// HTTP(S) URL of the MCP server endpoint
        url: String,

        /// Authentication method to use (optional, can configure later)
        #[arg(long, value_enum)]
        auth: Option<mcp::config::AuthType>,

        /// Environment variable name for 'env' auth type
        #[arg(long, requires = "auth")]
        auth_token: Option<String>,

        /// Keychain account name for 'keychain' auth type
        #[arg(long, requires = "auth")]
        auth_account: Option<String>,

        /// Command to execute for 'command' auth type
        #[arg(long, requires = "auth")]
        auth_command: Option<String>,
    },

    /// Remove an MCP server from the configuration
    #[command(
        long_about = "Remove a configured MCP server. This will delete the server configuration \
including any stored authentication credentials."
    )]
    Remove {
        /// Name of the server to remove
        name: String,
    },

    /// List all configured MCP servers and check their health
    #[command(
        long_about = "Display a list of all configured MCP servers showing their names, URLs, \
authentication status, and connection health. This command tests each server's connectivity."
    )]
    List,

    /// Get detailed information about an MCP server
    #[command(
        long_about = "Show detailed configuration for a specific MCP server including URL, \
authentication method, and connection status."
    )]
    Get {
        /// Name of the server to inspect
        name: String,
    },

    /// Configure or update authentication for a server
    #[command(
        long_about = "Interactively configure authentication for an MCP server. Supports:\n\n\
OAuth 2.1 (Recommended):\n\
  - Automatic discovery of authorization endpoints\n\
  - PKCE-protected authorization flow\n\
  - Automatic token refresh\n\
  - Full compliance with MCP authorization spec\n\n\
Environment Variable:\n\
  - Reference env var with ${VAR_NAME} syntax\n\
  - Useful for CI/CD and local development\n\n\
System Keychain:\n\
  - Secure storage in OS keychain\n\
  - Supports macOS Keychain, Windows Credential Manager, Linux Secret Service\n\n\
External Command:\n\
  - Run any command that outputs a token\n\
  - Example: op read op://vault/server/token"
    )]
    Auth {
        /// Name of the server to configure
        name: String,
    },
}

#[tokio::main]
async fn main() {
    // Install default crypto provider for rustls (required for TLS/HTTPS in Deno)
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let cli = Cli::parse();

    // Initialize logger
    utils::init_logger(cli.quiet, cli.verbose);

    let result = match &cli.command {
        Commands::Init => commands::init::handle(),
        Commands::Start { port, host } => commands::start::handle(host, *port).await,
        Commands::Mcp { mcp_cmd } => match mcp_cmd {
            McpCommands::Add {
                name,
                url,
                auth,
                auth_token,
                auth_account,
                auth_command,
            } => {
                commands::mcp_add::handle(
                    name,
                    url,
                    auth.as_ref(),
                    auth_token.as_deref(),
                    auth_account.as_deref(),
                    auth_command.as_deref(),
                )
                .await
            }
            McpCommands::Remove { name } => commands::mcp_remove::handle(name),
            McpCommands::List => commands::mcp_list::handle().await,
            McpCommands::Get { name } => commands::mcp_get::handle(name),
            McpCommands::Auth { name } => commands::mcp_auth::handle(name).await,
        },
    };

    if let Err(e) = result {
        error!("{e}");
        std::process::exit(1);
    }
}
