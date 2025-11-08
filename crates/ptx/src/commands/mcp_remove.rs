use anyhow::Result;
use log::info;

use crate::mcp::config::Config;

pub(crate) fn handle(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    config.remove_server(name)?;
    config.save()?;

    info!("✓ Removed server '{name}'");

    Ok(())
}
