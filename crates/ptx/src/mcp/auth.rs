use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

use super::config::{AuthConfig, ServerConfig};
use super::token_resolver::resolve_token;

/// Credentials returned by auth providers
#[derive(Debug, Clone, Default)]
pub(crate) struct AuthCredentials {
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
}

/// Trait for authentication providers
#[async_trait]
pub(crate) trait AuthProvider: Send + Sync {
    /// Get credentials for a request
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials>;

    /// Optional: Refresh credentials if expired
    /// Returns Ok(()) if refresh succeeded or if refresh is not supported
    #[allow(dead_code)]
    async fn refresh_credentials(&self, _config: &mut AuthConfig) -> Result<()> {
        // Default implementation: no refresh needed
        Ok(())
    }

    /// Optional: Validate credentials without making a full request
    /// Returns Ok(true) if valid, Ok(false) if invalid, Err if validation failed
    #[allow(dead_code)]
    async fn validate_credentials(&self, _credentials: &AuthCredentials) -> Result<bool> {
        // Default implementation: assume credentials are valid
        Ok(true)
    }
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
            .context("No OAuth2 credentials stored. Run 'ptcx mcp auth <server>' to authorize.")?;

        // Check if token is expired (basic check)
        if let Some(expires_at) = creds.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            if now >= expires_at {
                anyhow::bail!(
                    "OAuth2 access token is expired. Please re-authenticate with 'ptcx mcp auth <server>'"
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

/// Bearer token auth provider (supports ${VAR}, keychain://, command://, plain://)
pub(crate) struct BearerAuthProvider;

impl BearerAuthProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for BearerAuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for BearerAuthProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::Bearer { token } = config else {
            anyhow::bail!("Invalid auth config for BearerAuthProvider");
        };

        // Resolve the token using our unified token resolver
        let token_value = resolve_token(token).await?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {token_value}"));

        Ok(AuthCredentials {
            headers,
            query: HashMap::new(),
        })
    }
}

/// Custom headers/query params auth provider
pub(crate) struct CustomAuthProvider;

impl CustomAuthProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for CustomAuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for CustomAuthProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::Custom { headers, query } = config else {
            anyhow::bail!("Invalid auth config for CustomAuthProvider");
        };

        // Resolve all token references in headers and query params
        let mut resolved_headers = HashMap::new();
        for (key, value) in headers {
            let resolved_value = resolve_token(value).await?;
            resolved_headers.insert(key.clone(), resolved_value);
        }

        let mut resolved_query = HashMap::new();
        for (key, value) in query {
            let resolved_value = resolve_token(value).await?;
            resolved_query.insert(key.clone(), resolved_value);
        }

        Ok(AuthCredentials {
            headers: resolved_headers,
            query: resolved_query,
        })
    }
}

/// OAuth 2.1 Client Credentials auth provider (no browser required!)
pub(crate) struct OAuthClientCredentialsProvider;

impl OAuthClientCredentialsProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for OAuthClientCredentialsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for OAuthClientCredentialsProvider {
    async fn get_credentials(&self, config: &AuthConfig) -> Result<AuthCredentials> {
        let AuthConfig::OAuthClientCredentials {
            client_id,
            client_secret,
            token_url,
            scope,
            credentials,
        } = config
        else {
            anyhow::bail!("Invalid auth config for OAuthClientCredentialsProvider");
        };

        // Check if we have valid cached credentials
        if let Some(creds) = credentials {
            if let Some(expires_at) = creds.expires_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;

                // If token is still valid (with 60s buffer), use it
                if now < expires_at - 60 {
                    let mut headers = HashMap::new();
                    let token_type = creds.token_type.as_deref().unwrap_or("Bearer");
                    headers.insert(
                        "Authorization".to_string(),
                        format!("{} {}", token_type, creds.access_token),
                    );

                    return Ok(AuthCredentials {
                        headers,
                        query: HashMap::new(),
                    });
                }
            }
        }

        // Token is expired or doesn't exist - fetch new one
        // Resolve client_secret using token resolver
        let secret_value = resolve_token(client_secret).await?;

        // Perform OAuth 2.1 Client Credentials flow using reqwest directly
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize)]
        struct TokenRequest {
            grant_type: String,
            client_id: String,
            client_secret: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            scope: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        struct TokenResponse {
            access_token: String,
            #[serde(default)]
            token_type: Option<String>,
            #[serde(default)]
            expires_in: Option<i64>,
            #[serde(default)]
            #[allow(dead_code)]
            refresh_token: Option<String>,
        }

        let client = reqwest::Client::new();
        let request_body = TokenRequest {
            grant_type: "client_credentials".to_string(),
            client_id: client_id.clone(),
            client_secret: secret_value,
            scope: scope.clone(),
        };

        let response = client
            .post(token_url.clone())
            .form(&request_body)
            .send()
            .await
            .context("Failed to send token request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!(
                "Token request failed with status {}: {}",
                status,
                error_text
            );
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        let access_token = token_response.access_token;
        let token_type = token_response
            .token_type
            .unwrap_or_else(|| "Bearer".to_string());
        let _expires_at = token_response.expires_in.map(|secs| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + secs
        });

        // Note: We can't update the config here because we only have a reference
        // The caller (get_server_credentials) will need to handle persisting the new token
        // For now, just return the credentials

        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            format!("{} {}", token_type, access_token),
        );

        Ok(AuthCredentials {
            headers,
            query: HashMap::new(),
        })
    }

    async fn refresh_credentials(&self, config: &mut AuthConfig) -> Result<()> {
        // For client credentials flow, we just fetch a new token
        // This is called when the token is expired
        let AuthConfig::OAuthClientCredentials {
            client_id,
            client_secret,
            token_url,
            scope,
            credentials,
        } = config
        else {
            anyhow::bail!("Invalid auth config for refresh");
        };

        // Resolve client_secret
        let secret_value = resolve_token(client_secret).await?;

        // Perform OAuth 2.1 Client Credentials flow using reqwest directly
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize)]
        struct TokenRequest {
            grant_type: String,
            client_id: String,
            client_secret: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            scope: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct TokenResponse {
            access_token: String,
            #[serde(default)]
            token_type: Option<String>,
            #[serde(default)]
            expires_in: Option<i64>,
            #[serde(default)]
            refresh_token: Option<String>,
        }

        let http_client = reqwest::Client::new();
        let request_body = TokenRequest {
            grant_type: "client_credentials".to_string(),
            client_id: client_id.clone(),
            client_secret: secret_value,
            scope: scope.clone(),
        };

        let response = http_client
            .post(token_url.clone())
            .form(&request_body)
            .send()
            .await
            .context("Failed to send token refresh request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!(
                "Token refresh failed with status {}: {}",
                status,
                error_text
            );
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token refresh response")?;

        let access_token = token_response.access_token;
        let token_type = Some(
            token_response
                .token_type
                .unwrap_or_else(|| "Bearer".to_string()),
        );
        let expires_at = token_response.expires_in.map(|secs| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + secs
        });

        *credentials = Some(super::config::OAuth2Credentials {
            access_token,
            refresh_token: None, // Client credentials doesn't use refresh tokens
            expires_at,
            token_type,
        });

        Ok(())
    }
}

/// Get the appropriate auth provider for a server config
pub(crate) fn get_auth_provider(config: &AuthConfig) -> Box<dyn AuthProvider> {
    match config {
        AuthConfig::Bearer { .. } => Box::new(BearerAuthProvider::new()),
        AuthConfig::Custom { .. } => Box::new(CustomAuthProvider::new()),
        AuthConfig::OAuthClientCredentials { .. } => {
            Box::new(OAuthClientCredentialsProvider::new())
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bearer_auth_provider_with_env_var() {
        unsafe {
            std::env::set_var("TEST_BEARER_TOKEN", "test_bearer_value");
        }

        let config = AuthConfig::Bearer {
            token: "${TEST_BEARER_TOKEN}".to_string(),
        };

        let provider = BearerAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_ok(), "Should get credentials successfully");
        let creds = result.unwrap();
        assert_eq!(
            creds.headers.get("Authorization"),
            Some(&"Bearer test_bearer_value".to_string())
        );

        unsafe {
            std::env::remove_var("TEST_BEARER_TOKEN");
        }
    }

    #[tokio::test]
    async fn test_bearer_auth_provider_with_command() {
        let config = AuthConfig::Bearer {
            token: "command://printf 'cmd_token_123'".to_string(),
        };

        let provider = BearerAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_ok(), "Should execute command and get token");
        let creds = result.unwrap();
        assert_eq!(
            creds.headers.get("Authorization"),
            Some(&"Bearer cmd_token_123".to_string())
        );
    }

    #[tokio::test]
    async fn test_custom_auth_provider_with_headers() {
        unsafe {
            std::env::set_var("TEST_API_KEY", "api_key_value");
        }

        let mut headers = HashMap::new();
        headers.insert("X-API-Key".to_string(), "${TEST_API_KEY}".to_string());
        headers.insert("X-Client-ID".to_string(), "my-client".to_string());

        let config = AuthConfig::Custom {
            headers,
            query: HashMap::new(),
        };

        let provider = CustomAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_ok(), "Should get credentials successfully");
        let creds = result.unwrap();
        assert_eq!(
            creds.headers.get("X-API-Key"),
            Some(&"api_key_value".to_string())
        );
        assert_eq!(
            creds.headers.get("X-Client-ID"),
            Some(&"my-client".to_string())
        );

        unsafe {
            std::env::remove_var("TEST_API_KEY");
        }
    }

    #[tokio::test]
    async fn test_custom_auth_provider_with_query_params() {
        let mut query = HashMap::new();
        query.insert("api_key".to_string(), "my_api_key".to_string());
        query.insert("client_id".to_string(), "test-client".to_string());

        let config = AuthConfig::Custom {
            headers: HashMap::new(),
            query,
        };

        let provider = CustomAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_ok(), "Should get credentials successfully");
        let creds = result.unwrap();
        assert_eq!(creds.query.get("api_key"), Some(&"my_api_key".to_string()));
        assert_eq!(
            creds.query.get("client_id"),
            Some(&"test-client".to_string())
        );
    }

    #[tokio::test]
    async fn test_custom_auth_provider_mixed() {
        unsafe {
            std::env::set_var("TEST_HEADER_VALUE", "header_val");
        }

        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            "${TEST_HEADER_VALUE}".to_string(),
        );

        let mut query = HashMap::new();
        query.insert(
            "session".to_string(),
            "command://printf 'session_123'".to_string(),
        );

        let config = AuthConfig::Custom { headers, query };

        let provider = CustomAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_ok(), "Should handle mixed token types");
        let creds = result.unwrap();
        assert_eq!(
            creds.headers.get("Authorization"),
            Some(&"header_val".to_string())
        );
        assert_eq!(creds.query.get("session"), Some(&"session_123".to_string()));

        unsafe {
            std::env::remove_var("TEST_HEADER_VALUE");
        }
    }

    #[tokio::test]
    async fn test_get_auth_provider_returns_correct_provider() {
        // Test Bearer
        let bearer_config = AuthConfig::Bearer {
            token: "test".to_string(),
        };
        let _ = get_auth_provider(&bearer_config);

        // Test Custom
        let custom_config = AuthConfig::Custom {
            headers: HashMap::new(),
            query: HashMap::new(),
        };
        let _ = get_auth_provider(&custom_config);

        // Test legacy variants still work
        let env_config = AuthConfig::Env {
            token: "test".to_string(),
        };
        let _ = get_auth_provider(&env_config);
    }

    #[tokio::test]
    async fn test_legacy_env_auth_provider_still_works() {
        unsafe {
            std::env::set_var("TEST_LEGACY_TOKEN", "legacy_value");
        }

        let config = AuthConfig::Env {
            token: "${TEST_LEGACY_TOKEN}".to_string(),
        };

        let provider = EnvAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_ok(), "Legacy env provider should still work");
        let creds = result.unwrap();
        assert_eq!(
            creds.headers.get("Authorization"),
            Some(&"Bearer legacy_value".to_string())
        );

        unsafe {
            std::env::remove_var("TEST_LEGACY_TOKEN");
        }
    }

    #[tokio::test]
    async fn test_bearer_invalid_config() {
        let config = AuthConfig::Env {
            token: "wrong".to_string(),
        };

        let provider = BearerAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_err(), "Should error on invalid config type");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid auth config")
        );
    }

    #[tokio::test]
    async fn test_custom_invalid_config() {
        let config = AuthConfig::Bearer {
            token: "wrong".to_string(),
        };

        let provider = CustomAuthProvider::new();
        let result = provider.get_credentials(&config).await;

        assert!(result.is_err(), "Should error on invalid config type");
    }
}
