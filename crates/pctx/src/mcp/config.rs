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
    /// Bearer token (supports ${VAR}, keychain://, command://, plain://)
    Bearer { token: String },
    /// Custom headers and query parameters
    Custom {
        #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
        headers: std::collections::HashMap<String, String>,
        #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
        query: std::collections::HashMap<String, String>,
    },
    /// Legacy: Environment variable (migrated to Bearer)
    Env { token: String },
    /// Legacy: Keychain (migrated to Bearer with keychain://)
    Keychain { service: String, account: String },
    /// Legacy: Command (migrated to Bearer with command://)
    Command { command: String },
    /// OAuth 2.1 Client Credentials Flow (machine-to-machine)
    #[serde(rename = "oauth-client-credentials")]
    OAuthClientCredentials {
        client_id: String,
        client_secret: String, // Supports ${VAR}, keychain://, command://, plain://
        token_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        credentials: Option<OAuth2Credentials>,
    },
    /// NOT USABLE IN PRODUCTION OAuth 2.1 Authorization Code Flow (user-interactive, browser-based)
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
    Bearer,
    Custom,
    #[value(name = "oauth-client-credentials")]
    OAuthClientCredentials,
    Env,
    Keychain,
    Command,
    #[value(name = "oauth2")]
    OAuth2,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthType::Bearer => write!(f, "bearer"),
            AuthType::Custom => write!(f, "custom"),
            AuthType::OAuthClientCredentials => write!(f, "oauth-client-credentials"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_bearer_serialization() {
        let auth = AuthConfig::Bearer {
            token: "${MY_TOKEN}".to_string(),
        };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("type = \"bearer\""));
        assert!(toml.contains("token = \"${MY_TOKEN}\""));

        let deserialized: AuthConfig = toml::from_str(&toml).unwrap();
        matches!(deserialized, AuthConfig::Bearer { .. });
    }

    #[test]
    fn test_auth_config_custom_serialization() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("X-API-Key".to_string(), "${API_KEY}".to_string());

        let mut query = std::collections::HashMap::new();
        query.insert("client_id".to_string(), "my-client".to_string());

        let auth = AuthConfig::Custom { headers, query };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("type = \"custom\""));

        let deserialized: AuthConfig = toml::from_str(&toml).unwrap();
        matches!(deserialized, AuthConfig::Custom { .. });
    }

    #[test]
    fn test_auth_config_bearer_with_keychain() {
        let auth = AuthConfig::Bearer {
            token: "keychain://pctx/my-server".to_string(),
        };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("keychain://pctx/my-server"));
    }

    #[test]
    fn test_auth_config_bearer_with_command() {
        let auth = AuthConfig::Bearer {
            token: "command://op read op://vault/token".to_string(),
        };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("command://op read"));
    }

    #[test]
    fn test_auth_config_legacy_env_still_works() {
        let auth = AuthConfig::Env {
            token: "${OLD_TOKEN}".to_string(),
        };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("type = \"env\""));
    }

    #[test]
    fn test_server_config_roundtrip() {
        let mut server = ServerConfig::new(
            "test-server".to_string(),
            "https://api.example.com/mcp".to_string(),
        );

        server.auth = Some(AuthConfig::Bearer {
            token: "${TOKEN}".to_string(),
        });

        let toml = toml::to_string(&server).unwrap();
        let deserialized: ServerConfig = toml::from_str(&toml).unwrap();

        assert_eq!(deserialized.name, "test-server");
        assert_eq!(deserialized.url, "https://api.example.com/mcp");
        assert!(deserialized.auth.is_some());
    }

    #[test]
    fn test_oauth_client_credentials_serialization() {
        let auth = AuthConfig::OAuthClientCredentials {
            client_id: "my-client-id".to_string(),
            client_secret: "${CLIENT_SECRET}".to_string(),
            token_url: "https://auth.example.com/oauth/token".to_string(),
            scope: Some("api:read api:write".to_string()),
            credentials: None,
        };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("type = \"oauth-client-credentials\""));
        assert!(toml.contains("client_id = \"my-client-id\""));
        assert!(toml.contains("client_secret = \"${CLIENT_SECRET}\""));
        assert!(toml.contains("token_url = \"https://auth.example.com/oauth/token\""));
        assert!(toml.contains("scope = \"api:read api:write\""));

        let deserialized: AuthConfig = toml::from_str(&toml).unwrap();
        matches!(deserialized, AuthConfig::OAuthClientCredentials { .. });
    }

    #[test]
    fn test_oauth_client_credentials_with_stored_token() {
        let creds = OAuth2Credentials {
            access_token: "acc_token_123".to_string(),
            refresh_token: None,
            expires_at: Some(1_699_999_999),
            token_type: Some("Bearer".to_string()),
        };

        let auth = AuthConfig::OAuthClientCredentials {
            client_id: "my-client".to_string(),
            client_secret: "keychain://pctx/secret".to_string(),
            token_url: "https://auth.example.com/oauth/token".to_string(),
            scope: None,
            credentials: Some(creds),
        };

        let toml = toml::to_string(&auth).unwrap();
        assert!(toml.contains("access_token = \"acc_token_123\""));
        assert!(toml.contains("expires_at = 1699999999"));

        let deserialized: AuthConfig = toml::from_str(&toml).unwrap();
        if let AuthConfig::OAuthClientCredentials { credentials, .. } = deserialized {
            assert!(credentials.is_some());
            let creds = credentials.unwrap();
            assert_eq!(creds.access_token, "acc_token_123");
        } else {
            panic!("Expected OAuthClientCredentials");
        }
    }
}
