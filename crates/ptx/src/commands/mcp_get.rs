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
            AuthConfig::Env { token } => {
                info!("    Type: env");
                info!("    Token: {token}");
            }
            AuthConfig::Keychain { service, account } => {
                info!("    Type: keychain");
                info!("    Service: {service}");
                info!("    Account: {account}");
            }
            AuthConfig::Command { command } => {
                info!("    Type: command");
                info!("    Command: {command}");
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
