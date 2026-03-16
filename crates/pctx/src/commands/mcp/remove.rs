use anyhow::Result;
use clap::Parser;
use pctx_config::Config;
use tracing::info;

use crate::utils::styles::{fmt_bold, fmt_cyan_bold, fmt_good_check};

#[derive(Debug, Clone, Parser)]
pub struct RemoveCmd {
    /// Name of the server to remove
    pub name: String,
}

impl RemoveCmd {
    pub(crate) fn handle(&self, mut cfg: Config) -> Result<Config> {
        cfg.remove_server(&self.name)?;

        cfg.save()?;

        info!(
            "{}",
            fmt_good_check(&format!(
                "{name} MCP Server removed from {path}",
                name = fmt_bold(&self.name),
                path = fmt_cyan_bold(cfg.path().as_str()),
            ))
        );

        Ok(cfg)
    }
}
