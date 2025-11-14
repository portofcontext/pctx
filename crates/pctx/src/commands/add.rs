use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use log::info;

use crate::{
    commands::USER_CANCELLED,
    utils::{
        prompts,
        spinner::Spinner,
        styles::{fmt_bold, fmt_dimmed, fmt_success},
    },
};
use pctx_config::{
    Config,
    auth::{AuthConfig, SecretString},
    server::{McpConnectionError, ServerConfig},
};

#[derive(Debug, Clone, Parser)]
pub struct AddCmd {
    /// Unique name for this server
    pub name: String,

    /// HTTP(S) URL of the MCP server endpoint
    pub url: url::Url,

    /// use bearer authentication to connect to MCP server
    /// using PCTX's secret string syntax.
    ///
    /// e.g. `--bearer '${env:BEARER_TOKEN}'`
    #[arg(long, short, conflicts_with = "header")]
    pub bearer: Option<SecretString>,

    /// use custom headers to connect to MCP server
    /// using PCTX's secret string syntax. Many headers can
    /// be defined.
    ///
    /// e.g. `--headers 'x-api-key: ${keychain:API_KEY}'`
    #[arg(long, short = 'H')]
    pub header: Option<Vec<ClapHeader>>,

    /// Overrides any existing server under the same name &
    /// skips testing connection to the MCP server
    #[arg(long, short)]
    pub force: bool,
}

impl AddCmd {
    pub(crate) async fn handle(&self, mut cfg: Config, save: bool) -> Result<Config> {
        let mut server = ServerConfig::new(self.name.clone(), self.url.clone());

        // check for name clash
        if cfg.servers.iter().any(|s| s.name == server.name) {
            let re_add = self.force
                || inquire::Confirm::new(&format!(
                    "{} already exists, overwrite it?",
                    fmt_bold(&server.name)
                ))
                .with_default(false)
                .prompt()?;

            if !re_add {
                anyhow::bail!(USER_CANCELLED)
            }
        }

        // apply authentication (clap ensures bearer & header are mutually exclusive)
        server.auth = if let Some(bearer) = &self.bearer {
            Some(AuthConfig::Bearer {
                token: bearer.clone(),
            })
        } else if let Some(headers) = &self.header {
            Some(AuthConfig::Custom {
                headers: headers
                    .iter()
                    .map(|h| (h.name.clone(), h.value.clone()))
                    .collect(),
            })
        } else {
            let add_auth =
                inquire::Confirm::new("Do you want to add authentication interactively?")
                    .with_default(false)
                    .with_help_message(
                        "you can also manually update the auth configuration later in the config",
                    );
            if !self.force && add_auth.prompt()? {
                Some(prompts::prompt_auth(&server.name)?)
            } else {
                None
            }
        };

        // try connection
        if !self.force {
            let mut sp = Spinner::new("Testing MCP connection...");
            let connected = match server.connect().await {
                Ok(client) => {
                    sp.stop_success("Successfully connected");
                    client.cancel().await?;
                    true
                }
                Err(McpConnectionError::RequiresAuth) => {
                    sp.stop_and_persist(
                        "🔒",
                        if server.auth.is_none() {
                            "MCP requires authentication"
                        } else {
                            "Invalid authentication"
                        },
                    );
                    false
                }
                Err(McpConnectionError::Failed(msg)) => {
                    sp.stop_error(msg);
                    false
                }
            };

            if !connected {
                let add_anyway = inquire::Confirm::new(
                    "Do you still want to add the MCP server with the current settings?",
                )
                .with_default(false)
                .prompt()?;
                if !add_anyway {
                    anyhow::bail!(USER_CANCELLED)
                }
            }
        }

        cfg.add_server(server);

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

/// A header in the format "Name: value" where value is a `SecretString`
#[derive(Debug, Clone)]
pub struct ClapHeader {
    pub name: String,
    pub value: SecretString,
}

impl FromStr for ClapHeader {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (mut name, mut value) = s.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("Header must be in format '<HEADER NAME>: <SECRETS STRING>'")
        })?;
        if name.contains("${") {
            // edge case where the : is missing but exists in the secret string
            name = "";
            value = s;
        }

        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!(
                "Header name cannot be empty in format '<HEADER NAME>: <SECRETS STRING>'"
            );
        }

        let value_str = value.trim();
        if value_str.is_empty() {
            anyhow::bail!(
                "Header value cannot be empty in format '<HEADER NAME>: <SECRETS STRING>'"
            );
        }

        let value = SecretString::parse(value_str)?;

        Ok(ClapHeader {
            name: name.to_string(),
            value,
        })
    }
}
