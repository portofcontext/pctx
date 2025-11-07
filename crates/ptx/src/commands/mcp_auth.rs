use anyhow::{Context, Result};
use dialoguer::{Input, Select};
use log::{error, info};

use crate::mcp::{
    auth::store_in_keychain,
    config::{AuthConfig, Config, OAuth2Credentials},
};

pub(crate) async fn handle(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    let server = config
        .get_server_mut(name)
        .context(format!("Server '{name}' not found"))?;

    info!("Configuring authentication for '{name}'");
    info!("");

    let auth_methods = vec![
        "OAuth 2.1 (recommended for HTTP MCP servers)",
        "Environment variable",
        "System keychain",
        "External command",
    ];
    let selection = Select::new()
        .with_prompt("Authentication method?")
        .items(&auth_methods)
        .default(0)
        .interact()?;

    let auth_config = match selection {
        0 => {
            // OAuth 2.1
            info!("");
            info!("Starting OAuth 2.1 authorization flow...");
            info!("This will open your browser to authorize with the MCP server.");
            info!("");

            // Run the OAuth flow
            run_oauth_flow(&server.url).await?
        }
        1 => {
            // Environment variable
            let var_name: String = Input::new()
                .with_prompt("Environment variable name?")
                .interact_text()?;

            AuthConfig::Env {
                token: format!("${{{var_name}}}"),
            }
        }
        2 => {
            // System keychain
            let account: String = Input::new()
                .with_prompt("Keychain account name?")
                .with_initial_text(name)
                .interact_text()?;

            let token: String = Input::new()
                .with_prompt("Token to store?")
                .interact_text()?;

            // Store the token in the keychain
            store_in_keychain("pctl", &account, &token)?;

            info!("✓ Token stored in keychain");

            AuthConfig::Keychain {
                service: "pctl".to_string(),
                account,
            }
        }
        3 => {
            // External command
            let command: String = Input::new()
                .with_prompt("Command to run?")
                .with_initial_text("op read op://vault/mcp-server/token")
                .interact_text()?;

            AuthConfig::Command { command }
        }
        _ => unreachable!(),
    };

    server.auth = Some(auth_config);
    config.save()?;

    info!("");
    info!("✓ Authentication configured for '{name}'");

    Ok(())
}

const REDIRECT_URI: &str = "http://localhost:3000/callback";

/// OAuth callback data received from the authorization server
#[derive(Debug, Clone)]
struct OAuthCallback {
    code: String,
    state: String,
}

/// Run the OAuth 2.1 authorization flow using rmcp's `OAuthState`
async fn run_oauth_flow(server_url: &str) -> Result<AuthConfig> {
    use oauth2::TokenResponse;
    use rmcp::transport::auth::OAuthState;

    // Initialize OAuth state machine
    info!("Discovering OAuth configuration from server...");
    let mut oauth_state = OAuthState::new(server_url, None).await.context(
        "Failed to initialize OAuth state. The server may not support OAuth 2.1 or is unreachable.",
    )?;

    info!("✓ OAuth configuration discovered");
    info!("");

    // Determine scopes - we'll use empty slice to request all available scopes
    // (following MCP's scope selection strategy)
    let scopes: &[&str] = &[];

    // Start authorization (client_name is optional)
    oauth_state
        .start_authorization(scopes, REDIRECT_URI, Some("ptx"))
        .await
        .context("Failed to start authorization")?;

    // Get authorization URL
    let auth_url = oauth_state
        .get_authorization_url()
        .await
        .context("Failed to get authorization URL")?;

    info!("Starting local OAuth callback server on port 3000...");

    // Start the callback server and get the receiver
    let callback_rx = start_oauth_callback_server().await?;

    info!("Opening browser for authorization...");

    // Try to open the browser automatically
    if let Err(e) = open::that(&auth_url) {
        error!("Failed to open browser: {e}");
        info!("");
        info!("Please open this URL in your browser:");
        info!("  {auth_url}");
    }

    info!("");
    info!("Waiting for authorization callback...");

    // Wait for the callback
    let oauth_callback = callback_rx
        .await
        .context("Failed to receive OAuth callback")?;

    info!("✓ Received authorization callback");

    // Handle the callback
    info!("Exchanging authorization code for tokens...");

    oauth_state
        .handle_callback(&oauth_callback.code, &oauth_callback.state)
        .await
        .context("Failed to exchange authorization code for tokens")?;

    // Get credentials (client_id, token_response)
    let (client_id, token_response) = oauth_state
        .get_credentials()
        .await
        .context("Failed to get credentials from OAuth state")?;

    info!("✓ Successfully obtained access token!");
    info!("");

    // Extract access token and other information from the token response
    let token_resp = token_response.context("No token response available")?;
    let access_token = token_resp.access_token().secret().to_string();
    let refresh_token = token_resp.refresh_token().map(|t| t.secret().to_string());
    let expires_at = token_resp.expires_in().map(|duration| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + duration.as_secs() as i64
    });

    let oauth_creds = OAuth2Credentials {
        access_token,
        refresh_token,
        expires_at,
        token_type: Some("Bearer".to_string()),
    };

    Ok(AuthConfig::OAuth2 {
        client_id: Some(client_id),
        credentials: Some(oauth_creds),
    })
}

/// Start a local HTTP server to receive the OAuth callback
/// Returns a receiver that will receive the callback data when it arrives
async fn start_oauth_callback_server() -> Result<tokio::sync::oneshot::Receiver<OAuthCallback>> {
    use axum::{
        Router,
        extract::Query,
        response::{Html, IntoResponse},
        routing::get,
    };
    use std::sync::Arc;
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel::<OAuthCallback>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    let app = Router::new().route(
        "/callback",
        get({
            let tx = Arc::clone(&tx);
            move |Query(params): Query<std::collections::HashMap<String, String>>| async move {
                let code = params.get("code").cloned();
                let state = params.get("state").cloned();

                if let (Some(code), Some(state)) = (code, state) {
                    // Send the callback data
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(OAuthCallback { code, state });
                    }

                    Html(
                        r#"
                        <!DOCTYPE html>
                        <html>
                        <head>
                            <title>Authorization Successful</title>
                            <style>
                                body {
                                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                                    display: flex;
                                    justify-content: center;
                                    align-items: center;
                                    height: 100vh;
                                    margin: 0;
                                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                                }
                                .container {
                                    background: white;
                                    padding: 3rem;
                                    border-radius: 10px;
                                    box-shadow: 0 10px 40px rgba(0,0,0,0.2);
                                    text-align: center;
                                    max-width: 500px;
                                }
                                h1 {
                                    color: #2d3748;
                                    margin-bottom: 1rem;
                                }
                                p {
                                    color: #4a5568;
                                    font-size: 1.1rem;
                                    line-height: 1.6;
                                }
                                .success {
                                    font-size: 4rem;
                                    margin-bottom: 1rem;
                                }
                            </style>
                        </head>
                        <body>
                            <div class="container">
                                <div class="success">✓</div>
                                <h1>Authorization Successful!</h1>
                                <p>You have successfully authorized PTX.</p>
                                <p>You can close this window and return to your terminal.</p>
                            </div>
                        </body>
                        </html>
                        "#,
                    )
                    .into_response()
                } else {
                    Html(
                        r#"
                        <!DOCTYPE html>
                        <html>
                        <head>
                            <title>Authorization Failed</title>
                            <style>
                                body {
                                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                                    display: flex;
                                    justify-content: center;
                                    align-items: center;
                                    height: 100vh;
                                    margin: 0;
                                    background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
                                }
                                .container {
                                    background: white;
                                    padding: 3rem;
                                    border-radius: 10px;
                                    box-shadow: 0 10px 40px rgba(0,0,0,0.2);
                                    text-align: center;
                                    max-width: 500px;
                                }
                                h1 {
                                    color: #2d3748;
                                    margin-bottom: 1rem;
                                }
                                p {
                                    color: #4a5568;
                                    font-size: 1.1rem;
                                    line-height: 1.6;
                                }
                                .error {
                                    font-size: 4rem;
                                    margin-bottom: 1rem;
                                }
                            </style>
                        </head>
                        <body>
                            <div class="container">
                                <div class="error">✗</div>
                                <h1>Authorization Failed</h1>
                                <p>Missing authorization code or state parameter.</p>
                                <p>Please try again.</p>
                            </div>
                        </body>
                        </html>
                        "#,
                    )
                    .into_response()
                }
            }
        }),
    );

    // Spawn the server in a background task
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
            .await
            .context("Failed to bind to port 3000. Is another service using this port?")
            .unwrap();

        // Run the server - it will be gracefully shut down when the process exits
        let _ = axum::serve(listener, app).await;
    });

    // Wait a moment to ensure the server is listening
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    Ok(rx)
}
