use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

use super::config::{AuthConfig, ServerConfig};

/// Credentials returned by auth providers
#[derive(Debug, Clone, Default)]
pub(crate) struct AuthCredentials {
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
}

/// Trait for authentication providers
#[async_trait]
pub(crate) trait AuthProvider: Send + Sync {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials>;
}

/// Environment variable auth provider
pub(crate) struct EnvAuthProvider;

impl EnvAuthProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for EnvAuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for EnvAuthProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::Env { token } = config else {
            anyhow::bail!("Invalid auth config for EnvAuthProvider");
        };

        // Check if it's an environment variable reference (${VAR_NAME})
        let token_value = if token.starts_with("${") && token.ends_with('}') {
            let var_name = &token[2..token.len() - 1];
            std::env::var(var_name)
                .context(format!("Environment variable '{var_name}' not found"))?
        } else {
            // Use the token as-is (literal value)
            token.clone()
        };

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {token_value}"));

        Ok(AuthCredentials {
            headers,
            query: HashMap::new(),
        })
    }
}

/// System keychain auth provider
pub(crate) struct KeychainAuthProvider;

impl KeychainAuthProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for KeychainAuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for KeychainAuthProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::Keychain { service, account } = config else {
            anyhow::bail!("Invalid auth config for KeychainAuthProvider");
        };

        let entry =
            keyring::Entry::new(service, account).context("Failed to create keychain entry")?;

        let token = entry.get_password().context(format!(
            "Failed to get password from keychain for service '{service}', account '{account}'"
        ))?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));

        Ok(AuthCredentials {
            headers,
            query: HashMap::new(),
        })
    }
}

/// External command auth provider
pub(crate) struct CommandAuthProvider;

impl CommandAuthProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for CommandAuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for CommandAuthProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::Command { command } = config else {
            anyhow::bail!("Invalid auth config for CommandAuthProvider");
        };

        // Parse the command and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to execute auth command")?
            .wait_with_output()
            .await
            .context("Failed to wait for auth command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Auth command failed: {stderr}");
        }

        let token = String::from_utf8(output.stdout)
            .context("Auth command output is not valid UTF-8")?
            .trim()
            .to_string();

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));

        Ok(AuthCredentials {
            headers,
            query: HashMap::new(),
        })
    }
}

/// OAuth 2.1 auth provider (uses rmcp's `OAuthState`)
pub(crate) struct OAuth2AuthProvider;

impl OAuth2AuthProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for OAuth2AuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for OAuth2AuthProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::OAuth2 { credentials, .. } = config else {
            anyhow::bail!("Invalid auth config for OAuth2AuthProvider");
        };

        let creds = credentials
            .as_ref()
            .context("No OAuth2 credentials stored. Run 'ptx mcp auth <server>' to authorize.")?;

        // Check if token is expired (basic check)
        if let Some(expires_at) = creds.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            if now >= expires_at {
                anyhow::bail!(
                    "OAuth2 access token is expired. Please re-authenticate with 'ptx mcp auth <server>'"
                );
                // TODO: Implement automatic token refresh using refresh_token
            }
        }

        let mut headers = HashMap::new();
        let token_type = creds.token_type.as_deref().unwrap_or("Bearer");
        headers.insert(
            "Authorization".to_string(),
            format!("{} {}", token_type, creds.access_token),
        );

        Ok(AuthCredentials {
            headers,
            query: HashMap::new(),
        })
    }
}

/// Get the appropriate auth provider for a server config
pub(crate) fn get_auth_provider(config: &AuthConfig) -> Box<dyn AuthProvider> {
    match config {
        AuthConfig::Env { .. } => Box::new(EnvAuthProvider::new()),
        AuthConfig::Keychain { .. } => Box::new(KeychainAuthProvider::new()),
        AuthConfig::Command { .. } => Box::new(CommandAuthProvider::new()),
        AuthConfig::OAuth2 { .. } => Box::new(OAuth2AuthProvider::new()),
    }
}

/// Get credentials for a server
pub(crate) async fn get_server_credentials(
    server: &ServerConfig,
) -> Result<Option<AuthCredentials>> {
    if let Some(auth_config) = &server.auth {
        let provider = get_auth_provider(auth_config);
        let credentials = provider.get_credentials(auth_config).await?;
        Ok(Some(credentials))
    } else {
        Ok(None)
    }
}

/// Store a token in the system keychain
pub(crate) fn store_in_keychain(service: &str, account: &str, token: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, account).context("Failed to create keychain entry")?;

    entry
        .set_password(token)
        .context("Failed to store password in keychain")?;

    Ok(())
}
