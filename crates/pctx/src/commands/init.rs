use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use log::{info, warn};
use pctx_config::Config;

use crate::{
    commands::{USER_CANCELLED, add::AddCmd},
    utils::{
        prompts,
        styles::{fmt_bold, fmt_dimmed, fmt_success},
    },
};

#[derive(Debug, Clone, Parser)]
pub struct InitCmd {
    /// Use default values and skip interactive adding of upstream MCPs
    #[arg(long, short)]
    pub yes: bool,
}

impl InitCmd {
    pub(crate) async fn handle(&self, path: &Utf8PathBuf) -> Result<Config> {
        let mut cfg = if let Ok(_cfg) = Config::load(path) {
            let re_init = if self.yes {
                true
            } else {
                inquire::Confirm::new(&format!(
                    "A pctx config already exists at {}, overwrite it?",
                    fmt_dimmed(path.as_ref())
                ))
                .with_default(true)
                .prompt()?
            };
            if re_init {
                Config::default().with_path(path)
            } else {
                anyhow::bail!(USER_CANCELLED)
            }
        } else {
            Config::default().with_path(path)
        };

        let parent_name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
            .unwrap_or("root".into());

        if self.yes {
            cfg.name = parent_name;
        } else {
            cfg.name = inquire::Text::new("pctx name:")
                .with_validator(inquire::required!("name is required"))
                .with_default(&parent_name)
                .prompt()?;
            cfg.description =
                inquire::Text::new(&format!("pctx description {}:", fmt_dimmed("(optional)")))
                    .prompt_skippable()?;

            let mut add_upstream =
                inquire::Confirm::new("Would you like to add upstream MCP servers?")
                    .with_default(true)
                    .with_help_message(&format!(
                        "You can also do this later with {}",
                        fmt_bold("pctx add <NAME> <MCP_URL>")
                    ))
                    .prompt()?;

            while add_upstream {
                let name = inquire::Text::new("MCP name:")
                    .with_validator(inquire::required!())
                    .prompt()?;
                let url = inquire::Text::new("MCP URL:")
                    .with_validator(prompts::validators::url)
                    .prompt()?;
                let add_cmd = AddCmd {
                    name: name.clone(),
                    url: url.parse()?,
                    force: false,
                    bearer: None,
                    headers: None,
                };
                match add_cmd.handle(cfg.clone(), false).await {
                    Ok(updated) => {
                        cfg = updated;
                        info!(
                            "{}",
                            fmt_success(&format!("Added {name}", name = fmt_bold(&name)))
                        );
                    }
                    Err(e) => warn!("{e}"),
                }

                add_upstream = inquire::Confirm::new("Add another MCP server?")
                    .with_default(false)
                    .prompt()?;
            }
        }

        cfg.save()?;

        info!(
            "{}",
            fmt_success(&format!(
                "{name} configuration created: {path}",
                name = fmt_bold("pctx"),
                path = fmt_dimmed(cfg.path().as_str()),
            ))
        );

        Ok(cfg)
    }
}
