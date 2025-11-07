use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;

/// Resolve a token reference to its actual value
///
/// Supports multiple storage backends:
/// - `${VAR_NAME}` - Environment variable
/// - `keychain://service/account` - System keychain
/// - `command://shell command` - External command output
/// - Any other value - Treated as literal (backward compatibility)
pub(crate) async fn resolve_token(token_ref: &str) -> Result<String> {
    match token_ref {
        // Environment variable: ${VAR_NAME}
        ref_str if ref_str.starts_with("${") && ref_str.ends_with("}") => {
            let var_name = &ref_str[2..ref_str.len() - 1];
            std::env::var(var_name)
                .with_context(|| format!("Environment variable '{var_name}' not found"))
        }

        // Keychain: keychain://service/account
        ref_str if ref_str.starts_with("keychain://") => {
            let path = &ref_str[11..];
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() != 2 {
                anyhow::bail!(
                    "Invalid keychain reference format: '{ref_str}'. Expected 'keychain://service/account'"
                );
            }
            let entry = keyring::Entry::new(parts[0], parts[1])
                .context("Failed to create keychain entry")?;
            entry.get_password().with_context(|| {
                format!(
                    "Failed to retrieve password from keychain (service: '{}', account: '{}')",
                    parts[0], parts[1]
                )
            })
        }

        // External command: command://shell command here
        ref_str if ref_str.starts_with("command://") => {
            let command = &ref_str[10..];

            let output = Command::new("sh")
                .arg("-c")
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to spawn auth command")?
                .wait_with_output()
                .await
                .context("Failed to wait for auth command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Auth command failed: {}", stderr.trim());
            }

            let token = String::from_utf8(output.stdout)
                .context("Auth command output is not valid UTF-8")?
                .trim()
                .to_string();

            if token.is_empty() {
                anyhow::bail!("Auth command returned empty output");
            }

            Ok(token)
        }

        // Otherwise, treat as literal value (backward compatibility)
        ref_str => Ok(ref_str.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_token_env_var() {
        // Set up test environment variable
        unsafe {
            std::env::set_var("TEST_TOKEN_VAR", "test_token_value_123");
        }

        let result = resolve_token("${TEST_TOKEN_VAR}").await;
        assert!(result.is_ok(), "Should resolve env var successfully");
        assert_eq!(result.unwrap(), "test_token_value_123");

        // Clean up
        unsafe {
            std::env::remove_var("TEST_TOKEN_VAR");
        }
    }

    #[tokio::test]
    async fn test_resolve_token_env_var_missing() {
        let result = resolve_token("${NONEXISTENT_VAR_XYZ}").await;
        assert!(result.is_err(), "Should fail for missing env var");
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_resolve_token_command_success() {
        // Simple command that outputs a token (printf is more portable than echo -n)
        let result = resolve_token("command://printf 'my_secret_token'").await;
        assert!(result.is_ok(), "Should execute command successfully");
        assert_eq!(result.unwrap(), "my_secret_token");
    }

    #[tokio::test]
    async fn test_resolve_token_command_with_whitespace() {
        // Command that outputs token with surrounding whitespace (should be trimmed)
        let result = resolve_token("command://echo '  token_with_spaces  '").await;
        assert!(result.is_ok(), "Should execute command and trim output");
        assert_eq!(result.unwrap(), "token_with_spaces");
    }

    #[tokio::test]
    async fn test_resolve_token_command_failure() {
        // Command that exits with non-zero status
        let result = resolve_token("command://exit 1").await;
        assert!(
            result.is_err(),
            "Should fail for command with non-zero exit"
        );
    }

    #[tokio::test]
    async fn test_resolve_token_command_empty_output() {
        // Command that produces no output (true command exits successfully but outputs nothing)
        let result = resolve_token("command://true").await;
        assert!(result.is_err(), "Should fail for empty command output");
        assert!(result.unwrap_err().to_string().contains("empty output"));
    }

    #[tokio::test]
    async fn test_resolve_token_command_complex() {
        // More complex command with pipes
        let result = resolve_token("command://echo 'hello:world' | cut -d: -f2").await;
        assert!(result.is_ok(), "Should handle complex shell commands");
        assert_eq!(result.unwrap(), "world");
    }

    #[tokio::test]
    async fn test_resolve_token_literal_value() {
        // Backward compatibility: literal values without prefix
        let result = resolve_token("my_literal_token").await;
        assert!(result.is_ok(), "Should treat as literal value");
        assert_eq!(result.unwrap(), "my_literal_token");
    }

    #[tokio::test]
    async fn test_resolve_token_literal_with_special_chars() {
        // Literal value with special characters
        let result = resolve_token("sk_live_abc123_xyz789").await;
        assert!(result.is_ok(), "Should handle literal with special chars");
        assert_eq!(result.unwrap(), "sk_live_abc123_xyz789");
    }

    #[tokio::test]
    async fn test_resolve_token_keychain_invalid_format() {
        // Invalid keychain format (missing account)
        let result = resolve_token("keychain://service-only").await;
        assert!(result.is_err(), "Should fail for invalid keychain format");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid keychain reference")
        );
    }

    #[tokio::test]
    async fn test_resolve_token_keychain_too_many_parts() {
        // Invalid keychain format (too many parts)
        let result = resolve_token("keychain://service/account/extra").await;
        assert!(result.is_err(), "Should fail for too many keychain parts");
    }

    #[tokio::test]
    async fn test_resolve_token_env_var_malformed() {
        // Malformed env var (missing closing brace)
        let result = resolve_token("${UNCLOSED_VAR").await;
        assert!(result.is_ok(), "Should treat malformed as literal");
        assert_eq!(result.unwrap(), "${UNCLOSED_VAR");
    }

    #[tokio::test]
    async fn test_resolve_token_empty_string() {
        // Empty string
        let result = resolve_token("").await;
        assert!(result.is_ok(), "Should handle empty string");
        assert_eq!(result.unwrap(), "");
    }

    // Note: Keychain tests with actual keychain access are skipped
    // in CI environments. For local testing:
    //
    // #[tokio::test]
    // #[ignore] // Run with: cargo test -- --ignored
    // async fn test_resolve_token_keychain_success() {
    //     // First, store a test value
    //     use keyring::Entry;
    //     let entry = Entry::new("ptx-test", "test-account").unwrap();
    //     entry.set_password("test_keychain_value").unwrap();
    //
    //     let result = resolve_token("keychain://ptx-test/test-account").await;
    //     assert!(result.is_ok(), "Should resolve keychain successfully");
    //     assert_eq!(result.unwrap(), "test_keychain_value");
    //
    //     // Clean up
    //     entry.delete_password().ok();
    // }
}
