use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

pub use pctx_config::server::ServerConfig;

/// Simplified configuration for SDK use
///
/// This is a streamlined config focused on MCP server connections,
/// without CLI-specific concerns like logging, telemetry, versioning, etc.
///
/// Network access is automatically restricted to the hosts specified in the server URLs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SdkConfig {
    /// Upstream MCP server configurations
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
}

impl SdkConfig {
    /// Load configuration from a JSON file
    ///
    /// # Arguments
    /// * `path` - Path to the JSON config file
    ///
    /// # Errors
    /// Returns an error if the file doesn't exist or contains invalid JSON
    pub fn from_file(path: &str) -> Result<Self> {
        let contents =
            fs::read_to_string(path).context(format!("Failed to read config file: {path}"))?;

        let config: Self = serde_json::from_str(&contents)
            .context(format!("Failed to parse config file: {path}"))?;

        Ok(config)
    }

    /// Create config from a JSON string
    ///
    /// # Errors
    /// Returns an error if the JSON is invalid
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("Failed to parse config JSON")
    }

    /// Extract allowed hosts from server URLs
    ///
    /// Returns a list of host:port combinations derived from the server URLs.
    /// These are used for network access control in the sandbox.
    pub fn allowed_hosts(&self) -> Result<Vec<String>> {
        self.servers
            .iter()
            .map(|server| {
                let url = &server.url;

                let host = url
                    .host_str()
                    .ok_or_else(|| anyhow::anyhow!("No host in URL: {}", url))?;

                let host_with_port = if let Some(port) = url.port() {
                    format!("{}:{}", host, port)
                } else {
                    host.to_string()
                };

                Ok(host_with_port)
            })
            .collect()
    }
}

/// Convert from CLI Config to SDK Config
impl From<pctx_config::Config> for SdkConfig {
    fn from(config: pctx_config::Config) -> Self {
        Self {
            servers: config.servers,
        }
    }
}

/// Convert from SDK Config to CLI Config
impl From<SdkConfig> for pctx_config::Config {
    fn from(config: SdkConfig) -> Self {
        let mut cli_config = Self::default();
        cli_config.servers = config.servers;
        cli_config
    }
}
