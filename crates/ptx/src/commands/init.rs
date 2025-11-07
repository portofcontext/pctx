use anyhow::Result;
use log::{error, info};

use crate::mcp::config::Config;

pub(crate) fn handle() -> Result<()> {
    let config_path = Config::config_path()?;

    if config_path.exists() {
        error!("Configuration already exists at: {}", config_path.display());
        info!("Use 'pctx mcp add' to add MCP servers");
        return Ok(());
    }

    // Create a new empty config
    let config = Config::new();
    config.save()?;

    info!("✓ Configuration initialized at: {}", config_path.display());
    info!("");
    info!("Next steps:");
    info!("  - Add an MCP server: pctx mcp add <name> <url>");
    info!("  - Start the gateway: pctx start");

    Ok(())
}
