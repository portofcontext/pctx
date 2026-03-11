use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fmt::Display, fs};
use tracing::debug;

use crate::{logger::LoggerConfig, server::ServerConfig, telemetry::TelemetryConfig};

pub mod auth;
pub(crate) mod defaults;
pub mod logger;
pub mod server;
pub mod telemetry;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(skip_serializing)]
    path: Option<Utf8PathBuf>,

    /// Name of pctx mcp server
    pub name: String,

    /// Version of pctx mcp server
    #[serde(default = "default_version")]
    pub version: String,

    /// Description of the pctx mcp server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tool disclosure mode, determines which set of
    /// code mode tools will be made available.
    #[serde(default)]
    pub disclosure: ToolDisclosure,

    /// Upstream MCP server configurations
    #[serde(default)]
    pub servers: Vec<ServerConfig>,

    /// MCP server logger configuration
    #[serde(default)]
    pub logger: LoggerConfig,

    /// MCP server telemetry configuration
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

fn default_version() -> String {
    "0.1.0".into()
}

impl Config {
    #[must_use]
    pub fn with_path(mut self, path: &Utf8PathBuf) -> Self {
        self.path = Some(path.clone());
        self
    }

    pub fn path(&self) -> Utf8PathBuf {
        self.path.clone().unwrap_or(Self::default_path())
    }

    /// Loads config from json file, falling back on default path
    /// if none is provided
    ///
    /// # Errors
    ///
    /// This function will return an error if the config path does not exist or the content is invalid
    pub fn load(path: &Utf8PathBuf) -> Result<Self> {
        debug!("Loading config from {path}");

        if !path.exists() {
            anyhow::bail!("Config file does not exist: {path}");
        }

        let contents =
            fs::read_to_string(path).context(format!("Failed reading config: {path} "))?;

        let mut cfg: Self =
            serde_json::from_str(&contents).context(format!("Failed loading config: {path} "))?;
        cfg.path = Some(path.clone());

        Ok(cfg)
    }

    /// Saves config to json file, falling back on default path if non is provided
    ///
    /// # Errors
    /// This function will error if it fails writing the config
    pub fn save(&self) -> Result<()> {
        let dest = self.path();
        debug!("Saving config to {dest}");
        let contents = serde_json::to_string_pretty(self).unwrap_or(json!(self).to_string());

        fs::write(&dest, contents).context(format!("Failed writing config: {dest}"))?;

        Ok(())
    }

    /// Default config path is ./pctx.json
    pub fn default_path() -> Utf8PathBuf {
        Utf8PathBuf::new().join("pctx.json")
    }

    /// Adds server to the config
    pub fn add_server(&mut self, server: ServerConfig) -> bool {
        let orig_len = self.servers.len();

        // remove servers of matching names
        self.servers = self
            .servers
            .clone()
            .into_iter()
            .filter(|s| s.name != server.name)
            .collect();

        self.servers.push(server);
        orig_len != self.servers.len()
    }

    /// Removes server from the config
    ///
    /// # Errors
    ///
    /// This function will return an error if a server name does not exist
    pub fn remove_server(&mut self, name: &str) -> Result<()> {
        let index = self
            .servers
            .iter()
            .position(|s| s.name == name)
            .context(format!("Server '{name}' not found"))?;

        self.servers.remove(index);
        Ok(())
    }

    pub fn get_server(&self, name: &str) -> Option<&ServerConfig> {
        self.servers.iter().find(|s| s.name == name)
    }

    pub fn get_server_mut(&mut self, name: &str) -> Option<&mut ServerConfig> {
        self.servers.iter_mut().find(|s| s.name == name)
    }
}

#[derive(Copy, Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolDisclosure {
    /// list_tools -> get_tool_details -> execute_typescript
    #[default]
    #[serde(rename = "catalog")]
    Catalog,
    /// execute_bash -> execute_typescript
    #[serde(rename = "filesystem")]
    #[serde(alias = "fs")]
    Filesystem,
    /// original tool descriptions -> execute_typescript
    #[serde(rename = "sidecar")]
    Sidecar,
}
impl Display for ToolDisclosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = json!(self).to_string().replace("\"", "");
        write!(f, "{}", val)
    }
}
