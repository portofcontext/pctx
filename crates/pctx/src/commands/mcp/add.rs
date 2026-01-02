use std::{collections::BTreeMap, str::FromStr};

use anyhow::Result;
use clap::Parser;
use tracing::info;

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

    /// HTTP(S) URL of the MCP server endpoint (conflicts with --command for stdio)
    #[arg(conflicts_with_all = ["command", "args", "env"])]
    pub url: Option<url::Url>,

    /// Command to execute for stdio MCP server (conflicts with url)
    #[arg(long, conflicts_with = "url", requires = "name")]
    pub command: Option<String>,

    /// Arguments to pass to the stdio command (repeat for multiple)
    #[arg(long = "arg", requires = "command")]
    pub args: Vec<String>,

    /// Environment variables in KEY=VALUE format (repeat for multiple)
    #[arg(long = "env", requires = "command", value_parser = parse_env_var)]
    pub env: Vec<(String, String)>,

    /// use bearer authentication to connect to HTTP MCP server
    /// using PCTX's secret string syntax.
    ///
    /// e.g. `--bearer '${env:BEARER_TOKEN}'`
    #[arg(long, short, conflicts_with_all = ["header", "command"])]
    pub bearer: Option<SecretString>,

    /// use custom headers to connect to HTTP MCP server
    /// using PCTX's secret string syntax. Many headers can
    /// be defined.
    ///
    /// e.g. `--headers 'x-api-key: ${keychain:API_KEY}'`
    #[arg(long, short = 'H', conflicts_with = "command")]
    pub header: Option<Vec<ClapHeader>>,

    /// Overrides any existing server under the same name &
    /// skips testing connection to the MCP server
    #[arg(long, short)]
    pub force: bool,
}

fn parse_env_var(s: &str) -> Result<(String, String), String> {
    let (key, value) = s
        .split_once('=')
        .ok_or_else(|| "Env var must be in format 'KEY=VALUE'".to_string())?;
    let key = key.trim();
    if key.is_empty() {
        return Err("Env var key cannot be empty".to_string());
    }
    Ok((key.to_string(), value.to_string()))
}

impl AddCmd {
    pub(crate) async fn handle(&self, mut cfg: Config, save: bool) -> Result<Config> {
        // Create server config based on whether it's HTTP or stdio
        let mut server = if let Some(command) = &self.command {
            // Stdio mode
            let env = self.env.iter().cloned().collect::<BTreeMap<_, _>>();
            ServerConfig::new_stdio(self.name.clone(), command.clone(), self.args.clone(), env)
        } else if let Some(url) = &self.url {
            // HTTP mode
            ServerConfig::new(self.name.clone(), url.clone())
        } else {
            anyhow::bail!("Either --url or --command must be provided");
        };

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

        // apply authentication for HTTP servers only (clap ensures bearer & header are mutually exclusive)
        if server.http().is_some() {
            let auth = if let Some(bearer) = &self.bearer {
                Some(AuthConfig::Bearer {
                    token: bearer.clone(),
                })
            } else if let Some(headers) = &self.header {
                Some(AuthConfig::Headers {
                    headers: headers
                        .iter()
                        .map(|h| (h.name.clone(), h.value.clone()))
                        .collect(),
                })
            } else {
                let add_auth = inquire::Confirm::new(
                    "Do you want to add authentication interactively?",
                )
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
            server.set_auth(auth);
        }

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
                        "!",
                        if server.http().and_then(|cfg| cfg.auth.as_ref()).is_none() {
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

#[cfg(test)]
mod tests {
    use super::AddCmd;
    use pctx_config::Config;

    #[tokio::test]
    async fn test_add_http_server() {
        let cmd = AddCmd {
            name: "test-http".to_string(),
            url: Some("http://localhost:8080/mcp".parse().unwrap()),
            command: None,
            args: vec![],
            env: vec![],
            bearer: None,
            header: None,
            force: true,
        };

        let cfg = Config::default();
        let updated = cmd.handle(cfg, false).await.unwrap();
        let server = updated.get_server("test-http").expect("server added");

        assert!(server.http().is_some());
        assert!(server.stdio().is_none());
        assert_eq!(
            server.http().unwrap().url.as_str(),
            "http://localhost:8080/mcp"
        );
    }

    #[tokio::test]
    async fn test_add_stdio_server() {
        let cmd = AddCmd {
            name: "test-stdio".to_string(),
            url: None,
            command: Some("node".to_string()),
            args: vec!["./server.js".to_string()],
            env: vec![("NODE_ENV".to_string(), "test".to_string())],
            bearer: None,
            header: None,
            force: true,
        };

        let cfg = Config::default();
        let updated = cmd.handle(cfg, false).await.unwrap();
        let server = updated.get_server("test-stdio").expect("server added");

        assert!(server.stdio().is_some());
        assert!(server.http().is_none());

        let stdio = server.stdio().unwrap();
        assert_eq!(stdio.command, "node");
        assert_eq!(stdio.args, vec!["./server.js"]);
        assert_eq!(stdio.env.get("NODE_ENV").map(String::as_str), Some("test"));
    }

    #[tokio::test]
    async fn test_add_requires_url_or_command() {
        let cmd = AddCmd {
            name: "test".to_string(),
            url: None,
            command: None,
            args: vec![],
            env: vec![],
            bearer: None,
            header: None,
            force: true,
        };

        let cfg = Config::default();
        let result = cmd.handle(cfg, false).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Either --url or --command must be provided")
        );
    }
}
