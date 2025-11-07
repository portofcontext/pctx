use anyhow::{Context, Result};
use log::info;

use crate::mcp::config::{AuthConfig, Config};

pub(crate) fn handle(name: &str) -> Result<()> {
    let config = Config::load()?;

    let server = config
        .get_server(name)
        .context(format!("Server '{name}' not found"))?;

    info!("Server: {}", server.name);
    info!("  URL: {}", server.url);

    if let Some(auth) = &server.auth {
        info!("  Auth:");
        match auth {
            AuthConfig::Bearer { token } => {
                info!("    Type: bearer");
                info!("    Token: {token}");
            }
            AuthConfig::Custom { headers, query } => {
                info!("    Type: custom");
                if !headers.is_empty() {
                    info!("    Headers: {} configured", headers.len());
                }
                if !query.is_empty() {
                    info!("    Query params: {} configured", query.len());
                }
            }
            AuthConfig::Env { token } => {
                info!("    Type: env (legacy - consider migrating to bearer)");
                info!("    Token: {token}");
            }
            AuthConfig::Keychain { service, account } => {
                info!(
                    "    Type: keychain (legacy - consider migrating to bearer with keychain://)"
                );
                info!("    Service: {service}");
                info!("    Account: {account}");
            }
            AuthConfig::Command { command } => {
                info!("    Type: command (legacy - consider migrating to bearer with command://)");
                info!("    Command: {command}");
            }
            AuthConfig::OAuthClientCredentials {
                client_id,
                token_url,
                scope,
                credentials,
                ..
            } => {
                info!("    Type: oauth-client-credentials");
                info!("    Client ID: {client_id}");
                info!("    Token URL: {token_url}");
                if let Some(s) = scope {
                    info!("    Scope: {s}");
                }
                if let Some(creds) = credentials {
                    info!("    Status: token cached");
                    if let Some(expires_at) = creds.expires_at {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64;
                        if now < expires_at {
                            let remaining = expires_at - now;
                            info!("    Token expires in: {remaining}s");
                        } else {
                            info!("    Token: EXPIRED (will auto-refresh on next use)");
                        }
                    }
                } else {
                    info!("    Status: not yet fetched (will fetch on first use)");
                }
            }
            AuthConfig::OAuth2 {
                client_id,
                credentials,
            } => {
                info!("    Type: oauth2");
                if let Some(cid) = client_id {
                    info!("    Client ID: {cid}");
                }
                if let Some(creds) = credentials {
                    info!("    Status: authorized");
                    if let Some(expires_at) = creds.expires_at {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64;
                        if now < expires_at {
                            let remaining = expires_at - now;
                            info!("    Token expires in: {remaining}s");
                        } else {
                            info!("    Token: EXPIRED");
                        }
                    }
                } else {
                    info!("    Status: not authorized (run 'ptx mcp auth {name}')");
                }
            }
        }
    } else {
        info!("  Auth: none");
    }

    Ok(())
}
