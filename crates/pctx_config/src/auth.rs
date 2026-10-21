use std::{fmt::Display, process::Stdio, str::FromStr};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::process::Command;
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    /// Bearer token
    Bearer { token: SecretString },
    /// Custom headers
    #[serde(alias = "custom")] // "custom" alias for backwards compat.
    Headers {
        headers: IndexMap<String, SecretString>,
    },
    /// OAuth 2.1 Authorization Code + PKCE flow.
    ///
    /// All actual credentials (access token, refresh token, `client_id` /
    /// `client_secret`, token endpoint) live in the system keychain under
    /// `token_ref`; only non-secret metadata is persisted in `pctx.json`.
    /// `token_ref` is opaque — pctx generates one when the user runs
    /// `pctx mcp add` against an OAuth-protected server.
    #[serde(rename = "oauth")]
    OAuth {
        /// Keychain key holding the JSON-serialized [`crate::oauth2::TokenBundle`].
        token_ref: SecretString,
        /// Scopes that were granted at authorization time. Persisted so that
        /// `pctx mcp add` can re-authorize without prompting again, and so
        /// that observers reading `pctx.json` can see what access pctx has.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        scopes: Vec<String>,
    },
}

impl AuthConfig {
    /// Default keychain ref pctx uses for a server's OAuth token bundle.
    /// Stable so that re-running `pctx mcp add` for the same name overwrites
    /// the same entry instead of leaking old ones.
    pub fn default_oauth_token_ref(server_name: &str) -> SecretString {
        SecretString::new_plain(&format!("oauth:{server_name}"))
    }
}

/// A string that may contain 0 or more embedded secrets
/// Supports interpolation like "Bearer ${env:TOKEN}" or "plain text" or "prefix ${env:A} suffix ${keychain:B}"
#[derive(Debug, Clone)]
pub struct SecretString {
    parts: Vec<SecretPart>,
}

impl SecretString {
    pub fn new_plain(secret: &str) -> Self {
        Self {
            parts: vec![SecretPart::Plain(secret.into())],
        }
    }
    pub fn new_secret(secret: AuthSecret) -> Self {
        Self {
            parts: vec![SecretPart::Secret(secret)],
        }
    }
    pub fn new_parts(parts: Vec<SecretPart>) -> Self {
        Self { parts }
    }

    /// Parse secret string parts from string
    ///
    /// # Errors
    ///
    /// This function will return an error if the inputted string is not a valid secret string
    pub fn parse(input: &str) -> Result<Self> {
        let mut parts = Vec::new();
        let mut chars = input.char_indices().peekable();

        let mut current_plain = String::new();

        while let Some((i, ch)) = chars.next() {
            if ch == '$' {
                // Check if next char is '{'
                if let Some(&(_, '{')) = chars.peek() {
                    chars.next(); // consume '{'

                    // Save any accumulated plain text
                    if !current_plain.is_empty() {
                        parts.push(SecretPart::Plain(current_plain.clone()));
                        current_plain.clear();
                    }

                    // Find the closing '}'
                    let mut secret_content = String::new();
                    let mut found_closing = false;

                    for (_, ch) in chars.by_ref() {
                        if ch == '}' {
                            found_closing = true;
                            break;
                        }
                        secret_content.push(ch);
                    }

                    if !found_closing {
                        anyhow::bail!("Unclosed '${{' at position {i}");
                    }

                    if secret_content.is_empty() {
                        anyhow::bail!("Empty secret '${{}}' at position {i}");
                    }

                    // Parse the secret content: "prefix:value"
                    let secret = Self::parse_secret(&secret_content, i)?;
                    parts.push(SecretPart::Secret(secret));
                } else {
                    // Just a plain '$' without '{'
                    current_plain.push(ch);
                }
            } else if ch == '}' {
                // Unmatched closing brace
                anyhow::bail!("Unmatched '}}' at position {i}");
            } else {
                current_plain.push(ch);
            }
        }

        // Add any remaining plain text
        if !current_plain.is_empty() {
            parts.push(SecretPart::Plain(current_plain));
        }

        // If no parts, add empty plain string
        if parts.is_empty() {
            parts.push(SecretPart::Plain(String::new()));
        }

        Ok(SecretString { parts })
    }

    fn parse_secret(content: &str, pos: usize) -> Result<AuthSecret> {
        // Handle both formats: "env:VAR" and "VAR" (default to env)
        if let Some((prefix, value)) = content.split_once(':') {
            let value = value.trim();
            if value.is_empty() {
                anyhow::bail!("Empty secret value at position {pos}");
            }

            match prefix.trim() {
                "env" => Ok(AuthSecret::Env(value.to_string())),
                "keychain" => Ok(AuthSecret::Keychain(value.to_string())),
                "command" => Ok(AuthSecret::Command(value.to_string())),
                _ => anyhow::bail!("Unknown secret type '{prefix}' at position {pos}"),
            }
        } else {
            // No prefix, treat as environment variable
            let trimmed = content.trim();
            if trimmed.is_empty() {
                anyhow::bail!("Empty secret value at position {pos}");
            }
            Ok(AuthSecret::Env(trimmed.to_string()))
        }
    }

    pub fn parts(&self) -> &Vec<SecretPart> {
        &self.parts
    }

    pub fn keychain_keys(&self) -> Vec<String> {
        self.parts
            .iter()
            .filter_map(|p| {
                if let SecretPart::Secret(AuthSecret::Keychain(k)) = p {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if this string contains any secrets
    pub fn has_secrets(&self) -> bool {
        self.parts
            .iter()
            .any(|p| matches!(p, SecretPart::Secret(_)))
    }

    /// Returns the resolved `String` of the `SecretString`
    ///
    /// # Errors
    ///
    /// This function will return an error if any of the secret parts
    /// cannot resolve (e.g. Environment var not set)
    pub async fn resolve(&self) -> Result<String> {
        let mut resolved = String::new();

        for p in &self.parts {
            let val = match p {
                SecretPart::Plain(p) => p.clone(),
                SecretPart::Secret(auth_secret) => auth_secret.resolve().await?,
            };
            resolved = format!("{resolved}{val}");
        }

        Ok(resolved)
    }
}

impl Display for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = self
            .parts
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<String>();
        write!(f, "{val}")
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SecretString::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl FromStr for SecretString {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SecretString::parse(s)
    }
}

#[derive(Debug, Clone)]
pub enum SecretPart {
    /// Plain text segment
    Plain(String),
    /// Secret to be resolved
    Secret(AuthSecret),
}

impl Display for SecretPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretPart::Plain(str) => write!(f, "{str}"),
            SecretPart::Secret(secret) => write!(f, "${{{secret}}}"),
        }
    }
}

/// Authentication secret that supports multiple resolution strategies
#[derive(Debug, Clone)]
pub enum AuthSecret {
    /// Environment variable (matches: ${env:VAR})
    Env(String),
    /// macOS Keychain (matches: ${keychain:KEY})
    Keychain(String),
    /// Command execution (matches: ${command:npx keymanager keyname})
    Command(String),
}

impl AuthSecret {
    /// Returns the resolved string of the auth secret
    ///
    /// # Errors
    ///
    /// This function will return an error if the secrets can not be resolved from their sources (env/keyring/commands)
    pub async fn resolve(&self) -> Result<String> {
        match self {
            AuthSecret::Env(var) => std::env::var(var)
                .with_context(|| format!("Environment variable '{var}' not found")),

            AuthSecret::Keychain(key) => {
                let entry =
                    keyring::Entry::new("pctx", key).context("Failed to create keychain entry")?;
                entry.get_password().with_context(|| {
                    format!(
                        "Failed to retrieve password from keychain (service: 'pctx', user: '{key}')"
                    )
                })
            }
            AuthSecret::Command(cmd) => {
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .context(format!("Failed to spawn auth command: `{cmd}`"))?
                    .wait_with_output()
                    .await
                    .context(format!("Failed to wait for auth command: `{cmd}`"))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Auth command failed: `{cmd}`, stderr: {}", stderr.trim());
                }
                let token = String::from_utf8(output.stdout)
                    .context(format!("Auth command stdout is not valid UTF-8: `{cmd}`"))?
                    .trim()
                    .to_string();

                if token.is_empty() {
                    anyhow::bail!("Auth command returned empty output: `{cmd}`");
                }

                Ok(token)
            }
        }
    }
}

impl Display for AuthSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            AuthSecret::Env(var) => format!("env:{var}"),
            AuthSecret::Keychain(key) => format!("keychain:{key}"),
            AuthSecret::Command(cmd) => format!("command:{cmd}"),
        };

        write!(f, "{val}")
    }
}

/// Store a value in the system keychain as a password
///
/// # Errors
/// This function fails if keyring is unable to store a password
/// in the local system's keychain
pub fn write_to_keychain(key: &str, val: &str) -> Result<()> {
    let entry: keyring::Entry =
        keyring::Entry::new("pctx", key).context("Failed to create keychain entry")?;

    entry
        .set_password(val)
        .context("Failed to store password in keychain")?;

    debug!("Value stored in keychain service=\"pctx\", user=\"{key}\"");

    Ok(())
}

/// Removes a value stored in the system keychain as a password
///
/// # Errors
/// This function fails if keyring is unable to build the keychain entry
pub fn remove_from_keychain(key: &str) -> Result<()> {
    let entry: keyring::Entry =
        keyring::Entry::new("pctx", key).context("Failed to create keychain entry")?;

    match entry.delete_credential() {
        Ok(()) => (),
        Err(keyring::Error::NoEntry) => {
            debug!("No value stored in keychain matching service=\"pctx\", user=\"{key}\"");
        }
        Err(e) => anyhow::bail!(e),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let result = SecretString::parse("plain text").unwrap();
        assert_eq!(result.to_string(), "plain text");
        assert!(!result.has_secrets());
        assert_eq!(result.parts.len(), 1);
        assert!(matches!(result.parts[0], SecretPart::Plain(ref s) if s == "plain text"));
    }

    #[test]
    fn test_parse_empty_string() {
        let result = SecretString::parse("").unwrap();
        assert_eq!(result.to_string(), "");
        assert!(!result.has_secrets());
        assert_eq!(result.parts.len(), 1);
        assert!(matches!(result.parts[0], SecretPart::Plain(ref s) if s.is_empty()));
    }

    #[test]
    fn test_parse_env() {
        let result = SecretString::parse("Bearer ${env:TOKEN}").unwrap();
        assert_eq!(result.to_string(), "Bearer ${env:TOKEN}");
        assert!(result.has_secrets());
        assert_eq!(result.parts.len(), 2);
        assert!(matches!(result.parts[0], SecretPart::Plain(ref s) if s == "Bearer "));
        assert!(
            matches!(result.parts[1], SecretPart::Secret(AuthSecret::Env(ref s)) if s == "TOKEN")
        );
    }

    #[test]
    fn test_parse_keychain() {
        let result = SecretString::parse("${keychain:my-key}").unwrap();
        assert_eq!(result.to_string(), "${keychain:my-key}");
        assert!(result.has_secrets());
        assert_eq!(result.parts.len(), 1);
        assert!(
            matches!(result.parts[0], SecretPart::Secret(AuthSecret::Keychain(ref s)) if s == "my-key")
        );
    }

    #[test]
    fn test_parse_command() {
        let result = SecretString::parse("${command:npx get-token}").unwrap();
        assert_eq!(result.to_string(), "${command:npx get-token}");
        assert!(result.has_secrets());
        assert_eq!(result.parts.len(), 1);
        assert!(
            matches!(result.parts[0], SecretPart::Secret(AuthSecret::Command(ref s)) if s == "npx get-token")
        );
    }

    #[test]
    fn test_parse_multiple_secrets() {
        let result = SecretString::parse("prefix ${env:A} middle ${keychain:B} suffix").unwrap();
        assert!(result.has_secrets());
        assert_eq!(result.parts.len(), 5);
        assert!(matches!(result.parts[0], SecretPart::Plain(ref s) if s == "prefix "));
        assert!(matches!(result.parts[1], SecretPart::Secret(AuthSecret::Env(ref s)) if s == "A"));
        assert!(matches!(result.parts[2], SecretPart::Plain(ref s) if s == " middle "));
        assert!(
            matches!(result.parts[3], SecretPart::Secret(AuthSecret::Keychain(ref s)) if s == "B")
        );
        assert!(matches!(result.parts[4], SecretPart::Plain(ref s) if s == " suffix"));
    }

    #[test]
    fn test_parse_dollar_without_brace() {
        let result = SecretString::parse("Cost is $50").unwrap();
        assert_eq!(result.to_string(), "Cost is $50");
        assert!(!result.has_secrets());
        assert_eq!(result.parts.len(), 1);
        assert!(matches!(result.parts[0], SecretPart::Plain(ref s) if s == "Cost is $50"));
    }

    #[test]
    fn test_parse_unclosed_brace() {
        let result = SecretString::parse("Bearer ${TOKEN");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unclosed"));
    }

    #[test]
    fn test_parse_unmatched_closing_brace() {
        let result = SecretString::parse("Bearer }");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unmatched"));
    }

    #[test]
    fn test_parse_empty_secret() {
        let result = SecretString::parse("Bearer ${}");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty secret"));
    }

    #[test]
    fn test_parse_empty_secret_value() {
        let result = SecretString::parse("${env:}");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Empty secret value")
        );
    }

    #[test]
    fn test_parse_unknown_secret_type() {
        let result = SecretString::parse("${unknown:value}");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown secret type")
        );
    }

    #[test]
    fn test_parse_whitespace_handling() {
        let result = SecretString::parse("${  env : TOKEN  }").unwrap();
        assert!(result.has_secrets());
        assert_eq!(result.to_string(), "${env:TOKEN}");
        assert_eq!(result.parts.len(), 1);
        assert!(
            matches!(result.parts[0], SecretPart::Secret(AuthSecret::Env(ref s)) if s == "TOKEN")
        );
    }

    // === Resolution tests ===

    #[tokio::test]
    async fn test_resolve_env_var() {
        // Set up test environment variable
        unsafe {
            std::env::set_var("TEST_TOKEN_VAR", "test_token_value_123");
        }

        let secret = AuthSecret::Env("TEST_TOKEN_VAR".to_string());
        let result = secret.resolve().await;
        assert!(result.is_ok(), "Should resolve env var successfully");
        assert_eq!(result.unwrap(), "test_token_value_123");

        // Clean up
        unsafe {
            std::env::remove_var("TEST_TOKEN_VAR");
        }
    }

    #[tokio::test]
    async fn test_resolve_env_var_missing() {
        let secret = AuthSecret::Env("NONEXISTENT_VAR_XYZ".to_string());
        let result = secret.resolve().await;
        assert!(result.is_err(), "Should fail for missing env var");
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_resolve_command_success() {
        // Simple command that outputs a token (printf is more portable than echo -n)
        let secret = AuthSecret::Command("printf 'my_secret_token'".to_string());
        let result = secret.resolve().await;
        assert!(result.is_ok(), "Should execute command successfully");
        assert_eq!(result.unwrap(), "my_secret_token");
    }

    #[tokio::test]
    async fn test_resolve_command_with_whitespace() {
        // Command that outputs token with surrounding whitespace (should be trimmed)
        let secret = AuthSecret::Command("echo '  token_with_spaces  '".to_string());
        let result = secret.resolve().await;
        assert!(result.is_ok(), "Should execute command and trim output");
        assert_eq!(result.unwrap(), "token_with_spaces");
    }

    #[tokio::test]
    async fn test_resolve_command_failure() {
        // Command that exits with non-zero status
        let secret = AuthSecret::Command("exit 1".to_string());
        let result = secret.resolve().await;
        assert!(
            result.is_err(),
            "Should fail for command with non-zero exit"
        );
    }

    #[tokio::test]
    async fn test_resolve_command_empty_output() {
        // Command that produces no output (true command exits successfully but outputs nothing)
        let secret = AuthSecret::Command("true".to_string());
        let result = secret.resolve().await;
        assert!(result.is_err(), "Should fail for empty command output");
        assert!(result.unwrap_err().to_string().contains("empty output"));
    }

    #[tokio::test]
    async fn test_resolve_command_complex() {
        // More complex command with pipes
        let secret = AuthSecret::Command("echo 'hello:world' | cut -d: -f2".to_string());
        let result = secret.resolve().await;
        assert!(result.is_ok(), "Should handle complex shell commands");
        assert_eq!(result.unwrap(), "world");
    }

    #[tokio::test]
    async fn test_resolve_keychain_invalid_key() {
        // Try to resolve a keychain entry that doesn't exist
        let secret = AuthSecret::Keychain("nonexistent-test-key-xyz".to_string());
        let result = secret.resolve().await;
        assert!(result.is_err(), "Should fail for missing keychain entry");
    }

    // Note: Keychain tests with actual keychain access are skipped
    // in CI environments. For local testing:
    #[tokio::test]
    #[ignore = "keychain interaction"] // Run with: cargo test -- --ignored
    async fn test_resolve_keychain_success() {
        // First, store a test value
        let entry = keyring::Entry::new("pctx", "test-account").unwrap();
        entry.set_password("test_keychain_value").unwrap();

        let secret = AuthSecret::Keychain("test-account".to_string());
        let result = secret.resolve().await;
        assert!(result.is_ok(), "Should resolve keychain successfully");
        assert_eq!(result.unwrap(), "test_keychain_value");

        // Clean up
        entry.delete_credential().unwrap();
    }
}
