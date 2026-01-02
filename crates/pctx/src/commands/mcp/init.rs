use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use pctx_config::Config;
use tracing::{info, warn};

use crate::{
    commands::{USER_CANCELLED, mcp::add::AddCmd},
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
            cfg.version = "0.1.0".into();
        } else {
            cfg.name = inquire::Text::new("name:")
                .with_validator(inquire::required!("name is required"))
                .with_default(&parent_name)
                .prompt()?;
            cfg.version = inquire::Text::new("version:")
                .with_default("0.1.0")
                .with_validator(inquire::required!("version is required"))
                .prompt()?;
            cfg.description =
                inquire::Text::new(&format!("description {}:", fmt_dimmed("(optional)")))
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

                let transport_type = inquire::Select::new("Transport type:", vec!["HTTP", "stdio"])
                    .with_help_message(
                        "HTTP for network-based servers, stdio for local command-based servers",
                    )
                    .prompt()?;

                let add_cmd = if transport_type == "HTTP" {
                    let url = inquire::Text::new("MCP URL:")
                        .with_validator(prompts::validators::url)
                        .prompt()?;
                    AddCmd {
                        name: name.clone(),
                        url: Some(url.parse()?),
                        command: None,
                        args: vec![],
                        env: vec![],
                        force: false,
                        bearer: None,
                        header: None,
                    }
                } else {
                    // stdio
                    let command = inquire::Text::new("Command:")
                        .with_validator(inquire::required!())
                        .with_help_message("e.g., npx, node, python, etc.")
                        .prompt()?;

                    let args_input =
                        inquire::Text::new(&format!("Arguments {}:", fmt_dimmed("(optional)")))
                            .with_help_message("Space-separated arguments for the command")
                            .prompt_skippable()?;

                    let args = args_input
                        .map(|s| shlex::split(&s).unwrap_or_default())
                        .unwrap_or_default();

                    let add_env = inquire::Confirm::new("Add environment variables?")
                        .with_default(false)
                        .prompt()?;

                    let mut env = vec![];
                    if add_env {
                        loop {
                            let key = inquire::Text::new("Environment variable name:")
                                .with_validator(inquire::required!())
                                .prompt()?;
                            let value = inquire::Text::new(&format!("Value for {key}:"))
                                .with_validator(inquire::required!())
                                .prompt()?;
                            env.push((key, value));

                            let add_more =
                                inquire::Confirm::new("Add another environment variable?")
                                    .with_default(false)
                                    .prompt()?;
                            if !add_more {
                                break;
                            }
                        }
                    }

                    AddCmd {
                        name: name.clone(),
                        url: None,
                        command: Some(command),
                        args,
                        env,
                        force: false,
                        bearer: None,
                        header: None,
                    }
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
