use anyhow::Result;
use log::info;

use crate::mcp::PtcxMcp;
use crate::mcp::config::Config;
use crate::mcp::upstream::fetch_upstream_tools;

pub(crate) async fn handle(host: &str, port: u16) -> Result<()> {
    let config = Config::load()?;

    if config.servers.is_empty() {
        anyhow::bail!("No MCP servers configured. Add servers with 'pctx mcp add <name> <url>'");
    }

    info!("Starting Intelligent MCP gateway...");
    info!("");

    // Connect to each MCP server and fetch their tool definitions
    let mut upstream_servers = Vec::new();
    for server in &config.servers {
        info!("Connecting to '{}'...", server.name);
        match fetch_upstream_tools(server).await {
            Ok(upstream) => {
                info!("  ✓ Connected to '{}' at {}", server.name, server.url);
                upstream_servers.push(upstream);
            }
            Err(e) => {
                anyhow::bail!("Failed to connect to server '{}': {}", server.name, e);
            }
        }
    }

    info!("");
    info!("✓ Gateway starting on http://{host}:{port}");
    info!("✓ Configured servers:");
    for server in &config.servers {
        info!("  - {} ({})", server.name, server.url);
    }
    info!("");

    // Start the gateway with multiple MCP servers
    PtcxMcp::serve(host, port, upstream_servers).await;

    info!("Shutting down...");

    Ok(())
}
