use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// ``OAuth2`` credentials stored in config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OAuth2Credentials {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>, // Unix timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServerConfig {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum AuthConfig {
    Env {
        token: String,
    },
    Keychain {
        service: String,
        account: String,
    },
    Command {
        command: String,
    },
    #[serde(rename = "oauth2")]
    OAuth2 {
        /// Optional client ID (stored after dynamic registration)
        #[serde(skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
        /// Stored ``OAuth2`` credentials (managed internally)
        #[serde(skip_serializing_if = "Option::is_none")]
        credentials: Option<OAuth2Credentials>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum AuthType {
    Env,
    Keychain,
    Command,
    #[value(name = "oauth2")]
    OAuth2,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthType::Env => write!(f, "env"),
            AuthType::Keychain => write!(f, "keychain"),
            AuthType::Command => write!(f, "command"),
            AuthType::OAuth2 => write!(f, "oauth2"),
        }
    }
}

impl Config {
    pub(crate) fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub(crate) fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::new());
        }

        let contents = fs::read_to_string(&config_path).context("Failed to read config file")?;

        toml::from_str(&contents).context("Failed to parse config file")
    }

    pub(crate) fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, contents).context("Failed to write config file")?;

        Ok(())
    }

    pub(crate) fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to determine home directory")?;

        Ok(home.join(".pctx").join("config.toml"))
    }

    pub(crate) fn add_server(&mut self, server: ServerConfig) -> Result<()> {
        if self.servers.iter().any(|s| s.name == server.name) {
            anyhow::bail!("Server '{}' already exists", server.name);
        }

        self.servers.push(server);
        Ok(())
    }

    pub(crate) fn remove_server(&mut self, name: &str) -> Result<()> {
        let index = self
            .servers
            .iter()
            .position(|s| s.name == name)
            .context(format!("Server '{name}' not found"))?;

        self.servers.remove(index);
        Ok(())
    }

    pub(crate) fn get_server(&self, name: &str) -> Option<&ServerConfig> {
        self.servers.iter().find(|s| s.name == name)
    }

    pub(crate) fn get_server_mut(&mut self, name: &str) -> Option<&mut ServerConfig> {
        self.servers.iter_mut().find(|s| s.name == name)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerConfig {
    pub(crate) fn new(name: String, url: String) -> Self {
        Self {
            name,
            url,
            auth: None,
        }
    }
}
