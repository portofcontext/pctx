use anyhow::{Context, Result};
use dialoguer::{Input, Select};
use log::info;

use crate::mcp::{
    auth::store_in_keychain,
    config::{AuthConfig, Config},
};

pub(crate) fn handle(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    let server = config
        .get_server_mut(name)
        .context(format!("Server '{name}' not found"))?;

    info!("Configuring authentication for '{name}'");
    info!("");

    // First, ask what type of credentials they have
    let credential_types = vec![
        "Bearer token / API key",
        "OAuth Client Credentials (client_id + client_secret)",
    ];

    let cred_selection = Select::new()
        .with_prompt("What type of credentials do you have?")
        .items(&credential_types)
        .default(0)
        .interact()?;

    let auth_config = match cred_selection {
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

                    AuthConfig::Bearer {
                        token: format!("keychain://pctx/{account}"),
                    }
                }
                1 => {
                    // Environment variable
                    let var_name: String = Input::new()
                        .with_prompt("Environment variable name?")
                        .interact_text()?;

                    AuthConfig::Bearer {
                        token: format!("${{{var_name}}}"),
                    }
                }
                2 => {
                    // External command
                    let command: String = Input::new()
                        .with_prompt("Command to run?")
                        .with_initial_text("op read op://vault/mcp-server/token")
                        .interact_text()?;

                    AuthConfig::Bearer {
                        token: format!("command://{command}"),
                    }
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
                        .with_initial_text(format!("{name}-client-secret"))
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

            AuthConfig::OAuthClientCredentials {
                client_id,
                client_secret,
                token_url,
                scope: if scope.is_empty() { None } else { Some(scope) },
                credentials: None,
            }
        }
        _ => unreachable!(),
    };

    server.auth = Some(auth_config);
    config.save()?;

    info!("");
    info!("✓ Authentication configured for '{name}'");

    Ok(())
}
