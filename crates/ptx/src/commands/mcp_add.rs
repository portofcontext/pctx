use anyhow::{Context, Result};
use dialoguer::{Input, Select};
use log::info;

use crate::mcp::{
    auth::store_in_keychain,
    client::{InitMCPClientError, init_mcp_client},
    config::{AuthConfig, AuthType, Config, ServerConfig},
};

pub(crate) async fn handle(
    name: &str,
    url: &str,
    auth_type: Option<&AuthType>,
    auth_token: Option<&str>,
    auth_account: Option<&str>,
    auth_command: Option<&str>,
) -> Result<()> {
    let mut config = Config::load()?;

    // If no auth specified via CLI, test the server to see if it needs auth
    let auth_config = if let Some(auth_type) = auth_type {
        // CLI auth was specified - use it
        Some(create_auth_config(
            *auth_type,
            auth_token,
            auth_account,
            auth_command,
        )?)
    } else {
        // No CLI auth - test the server and prompt if needed
        info!("Testing connection to '{url}'...");
        match init_mcp_client(url).await {
            Ok(client) => {
                info!("✓ Successfully connected without authentication");
                client.cancel().await?;
                None
            }
            Err(InitMCPClientError::RequiresOAuth) => {
                info!("✓ Server supports OAuth 2.1");
                info!("");
                if let Ok(auth) = prompt_for_auth(name) {
                    Some(auth)
                } else {
                    info!("You can configure authentication later with: ptx mcp auth {name}");
                    None
                }
            }
            Err(InitMCPClientError::RequiresAuth) => {
                info!("⚠ Server requires authentication");
                info!("");
                if let Ok(auth) = prompt_for_auth(name) {
                    Some(auth)
                } else {
                    info!("You can configure authentication later with: ptx mcp auth {name}");
                    None
                }
            }
            Err(InitMCPClientError::Failed(reason)) => {
                info!("⚠ Connection test failed: {reason}");
                info!("  Adding server anyway - you can test it later.");
                None
            }
        }
    };

    let mut server = ServerConfig::new(name.to_string(), url.to_string());
    server.auth = auth_config;

    config.add_server(server)?;
    config.save()?;

    info!("");
    info!("✓ Added server '{name}'");
    info!("  URL: {url}");
    if let Some(ref auth) = config.get_server(name).and_then(|s| s.auth.as_ref()) {
        let auth_type = match auth {
            AuthConfig::Bearer { .. } => "bearer",
            AuthConfig::Custom { .. } => "custom",
            AuthConfig::OAuthClientCredentials { .. } => "oauth-client-credentials",
            AuthConfig::Env { .. } => "env",
            AuthConfig::Keychain { .. } => "keychain",
            AuthConfig::Command { .. } => "command",
            AuthConfig::OAuth2 { .. } => "oauth2",
        };
        info!("  Auth: {auth_type}");
    }

    Ok(())
}

/// Create auth config from CLI arguments
fn create_auth_config(
    auth_type: AuthType,
    auth_token: Option<&str>,
    auth_account: Option<&str>,
    auth_command: Option<&str>,
) -> Result<AuthConfig> {
    Ok(match auth_type {
        AuthType::Bearer => {
            let token = auth_token.context("--auth-token is required for bearer auth")?;
            AuthConfig::Bearer {
                token: token.to_string(),
            }
        }
        AuthType::Custom => {
            // Custom auth requires headers/query - redirect to interactive config
            anyhow::bail!(
                "Custom auth type requires interactive configuration. Use 'ptx mcp auth <name>' command."
            );
        }
        AuthType::OAuthClientCredentials => {
            // OAuth Client Credentials requires interactive setup for client_id, client_secret, token_url
            anyhow::bail!(
                "OAuth Client Credentials requires interactive configuration. Use 'ptx mcp auth <name>' command."
            );
        }
        AuthType::Env => {
            let token = auth_token.context("--auth-token is required for env auth")?;
            AuthConfig::Env {
                token: token.to_string(),
            }
        }
        AuthType::Keychain => {
            let account = auth_account.context("--auth-account is required for keychain auth")?;
            AuthConfig::Keychain {
                service: "pctx".to_string(),
                account: account.to_string(),
            }
        }
        AuthType::Command => {
            let command = auth_command.context("--auth-command is required for command auth")?;
            AuthConfig::Command {
                command: command.to_string(),
            }
        }
        AuthType::OAuth2 => {
            // OAuth2 is configured via `ptx mcp auth <name>` command
            AuthConfig::OAuth2 {
                client_id: None,
                credentials: None,
            }
        }
    })
}

/// Prompt user to select and configure authentication method
fn prompt_for_auth(name: &str) -> Result<AuthConfig> {
    // First, ask what type of credentials they have
    let credential_types = vec![
        "Bearer token / API key",
        "OAuth Client Credentials (client_id + client_secret)",
        "Skip for now",
    ];

    let cred_selection = Select::new()
        .with_prompt("What type of credentials do you have?")
        .items(&credential_types)
        .default(0)
        .interact()?;

    match cred_selection {
        0 => {
            // Bearer token - now ask how they want to provide it
            let storage_methods = vec![
                "Enter it now (store in system keychain)",
                "Environment variable",
                "External command (e.g., 1Password, AWS Secrets Manager)",
            ];

            let storage_selection = Select::new()
                .with_prompt("How do you want to provide the token?")
                .items(&storage_methods)
                .default(0)
                .interact()?;

            match storage_selection {
                0 => {
                    // Store in keychain
                    let account: String = Input::new()
                        .with_prompt("Keychain account name?")
                        .with_initial_text(name)
                        .interact_text()?;

                    let token: String = Input::new().with_prompt("Token value?").interact_text()?;

                    // Store the token in the keychain
                    store_in_keychain("pctx", &account, &token)?;

                    info!("✓ Token stored in keychain");

                    Ok(AuthConfig::Bearer {
                        token: format!("keychain://pctx/{account}"),
                    })
                }
                1 => {
                    // Environment variable
                    let var_name: String = Input::new()
                        .with_prompt("Environment variable name?")
                        .interact_text()?;

                    Ok(AuthConfig::Bearer {
                        token: format!("${{{var_name}}}"),
                    })
                }
                2 => {
                    // External command
                    let command: String = Input::new()
                        .with_prompt("Command to run?")
                        .with_initial_text("op read op://vault/mcp-server/token")
                        .interact_text()?;

                    Ok(AuthConfig::Bearer {
                        token: format!("command://{command}"),
                    })
                }
                _ => unreachable!(),
            }
        }
        1 => {
            // OAuth Client Credentials
            let client_id: String = Input::new().with_prompt("Client ID?").interact_text()?;

            let token_url: String = Input::new().with_prompt("Token URL?").interact_text()?;

            // Ask how they want to provide the client secret
            let storage_methods = vec![
                "Store in system keychain",
                "Environment variable",
                "External command (e.g., 1Password, AWS Secrets Manager)",
            ];

            let storage_selection = Select::new()
                .with_prompt("How do you want to provide the client secret? (It will automatically pull from the chosen location)")
                .items(&storage_methods)
                .default(0)
                .interact()?;

            let client_secret = match storage_selection {
                0 => {
                    // Store in keychain
                    let account: String = Input::new()
                        .with_prompt("Keychain account name?")
                        .with_initial_text(&format!("{}-client-secret", name))
                        .interact_text()?;

                    let secret: String = Input::new()
                        .with_prompt("Client secret value?")
                        .interact_text()?;

                    // Store the secret in the keychain
                    store_in_keychain("pctx", &account, &secret)?;

                    info!("✓ Client secret stored in keychain");

                    format!("keychain://pctx/{account}")
                }
                1 => {
                    // Environment variable
                    let var_name: String = Input::new()
                        .with_prompt("Environment variable name?")
                        .interact_text()?;

                    format!("${{{var_name}}}")
                }
                2 => {
                    // External command
                    let command: String = Input::new()
                        .with_prompt("Command to run?")
                        .with_initial_text("op read op://vault/mcp-server/client-secret")
                        .interact_text()?;

                    format!("command://{command}")
                }
                _ => unreachable!(),
            };

            let scope: String = Input::new()
                .with_prompt("Scope (optional, press Enter to skip)?")
                .allow_empty(true)
                .interact_text()?;

            Ok(AuthConfig::OAuthClientCredentials {
                client_id,
                client_secret,
                token_url,
                scope: if scope.is_empty() { None } else { Some(scope) },
                credentials: None,
            })
        }
        2 => {
            // Skip for now
            anyhow::bail!("Authentication skipped")
        }
        _ => unreachable!(),
    }
}
