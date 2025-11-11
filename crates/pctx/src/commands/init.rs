use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use log::info;
use pctx_config::Config;

use crate::{
    commands::add::AddCmd,
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
                anyhow::bail!("User cancelled")
            }
        } else {
            Config::default().with_path(path)
        };

        let parent_name = Utf8PathBuf::new()
            .parent()
            .and_then(|p| p.file_name().map(ToString::to_string));

        if self.yes {
            cfg.name = parent_name.unwrap_or("root".into());
        } else {
            let mut name_input = inquire::Text::new("pctx name:")
                .with_validator(inquire::required!("name is required"));
            if let Some(p_name) = &parent_name {
                name_input = name_input.with_default(p_name);
            }
            cfg.name = name_input.prompt()?;
            cfg.description = inquire::Text::new("pctx description:").prompt_skippable()?;

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
                };
                cfg = add_cmd.handle(cfg, false).await?;
                info!(
                    "{}",
                    fmt_success(&format!("Added {name}", name = fmt_bold(&name)))
                );

                add_upstream = inquire::Confirm::new("Add another?")
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
