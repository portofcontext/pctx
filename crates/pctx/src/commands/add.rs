use anyhow::Result;
use async_recursion::async_recursion;
use clap::Parser;
use log::info;

use crate::utils::{
    prompts,
    spinner::Spinner,
    styles::{fmt_bold, fmt_dimmed, fmt_success},
};
use pctx_config::{
    Config,
    server::{McpConnectionError, ServerConfig},
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct AddCmd {
    /// Unique name for this server
    pub(crate) name: String,

    /// HTTP(S) URL of the MCP server endpoint
    pub(crate) url: url::Url,

    /// Overrides any existing server under the same name &
    /// skips testing connection to the MCP server
    #[arg(long, short)]
    pub(crate) force: bool,
}

impl AddCmd {
    pub(crate) async fn handle(&self, mut cfg: Config, save: bool) -> Result<Config> {
        let mut server_cfg = ServerConfig::new(self.name.clone(), self.url.clone());

        if !self.force {
            server_cfg = try_connection(server_cfg, true).await?;
        }

        cfg.add_server(server_cfg, self.force)?;

        if save {
            cfg.save()?;
            info!(
                "{}",
                fmt_success(&format!(
                    "{name} upstream MCP added to {path}",
                    name = fmt_bold(&self.name),
                    path = fmt_dimmed(cfg.path().as_str()),
                ))
            );
        }

        Ok(cfg)
    }
}

#[async_recursion]
async fn try_connection(mut server: ServerConfig, first_attempt: bool) -> Result<ServerConfig> {
    let mut sp = Spinner::new(if first_attempt {
        "Testing MCP connection..."
    } else {
        "Retrying MCP connection..."
    });

    match server.connect().await {
        Ok(client) => {
            sp.stop_success("Successfully connected");
            client.cancel().await?;
        }
        Err(McpConnectionError::RequiresAuth) => {
            sp.stop_and_persist(
                "🔒",
                if first_attempt {
                    "MCP requires authentication"
                } else {
                    "Invalid authentication"
                },
            );
            let add_auth = inquire::Confirm::new(if first_attempt {
                "Do you want to add authentication interactively?"
            } else {
                "Do you want to update authentication interactively?"
            })
            .with_default(true)
            .with_help_message(
                "you can also manually update the auth configuration later in the config",
            )
            .prompt()?;

            if add_auth {
                server.auth = Some(prompts::prompt_auth(&server.name)?);
                return try_connection(server, false).await;
            }
        }
        Err(McpConnectionError::Failed(msg)) => {
            sp.stop_error(msg);
            let add_anyway = inquire::Confirm::new("Do you still want to add the MCP server?")
                .with_default(false)
                .prompt()?;
            if !add_anyway {
                anyhow::bail!("User cancelled")
            }
        }
    }

    Ok(server)
}
