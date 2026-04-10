//! Interactive OAuth 2.1 authorization-code flow for `pctx mcp add`.
//!
//! This module owns the user-facing parts of the OAuth flow: spinning up a
//! one-shot localhost callback listener, opening the user's browser, waiting
//! for the redirect, and exchanging the authorization code for tokens. The
//! lower-level protocol bits (discovery, PKCE, token exchange, refresh) live
//! in [`pctx_config::oauth2`] so they can be reused outside the CLI.

use std::{
    net::{Ipv4Addr, SocketAddrV4, TcpListener as StdTcpListener},
    time::Duration,
};

use anyhow::{Context, Result};
use pctx_config::{
    auth::AuthConfig,
    oauth2::{self, OAuthMetadata, Pkce},
};
use tracing::{debug, info, warn};

use crate::utils::styles::{fmt_cyan_bold, fmt_dimmed, fmt_good_check};

/// Default scopes pctx requests when the auth-server metadata advertises
/// `scopes_supported` — we just ask for whatever the server lists. If the
/// server doesn't advertise any, we send no `scope` parameter and let the
/// server use its defaults.
fn pick_scopes(metadata: &OAuthMetadata) -> Vec<String> {
    metadata.scopes_supported.clone()
}

/// Run the full interactive OAuth 2.1 flow for an MCP server, returning a
/// ready-to-persist [`AuthConfig::OAuth`] (with the token bundle already
/// stored in the system keychain).
///
/// `server_name` is used to derive the keychain `token_ref` and to label the
/// dynamically registered client.
///
/// # Errors
/// Returns an error if discovery fails, the user cancels, the browser flow
/// times out, or the token endpoint returns a non-success response.
pub(crate) async fn run_interactive_flow(
    server_name: &str,
    server_url: &url::Url,
) -> Result<AuthConfig> {
    info!(
        "{}",
        fmt_dimmed(&format!("Discovering OAuth metadata for {server_url}..."))
    );
    let metadata = oauth2::discover(server_url)
        .await
        .context("OAuth discovery request failed")?
        .ok_or_else(|| {
            anyhow::anyhow!("Server does not advertise OAuth metadata at any well-known endpoint")
        })?;
    debug!(
        "OAuth metadata: authorize={} token={} registration={:?}",
        metadata.authorization_endpoint, metadata.token_endpoint, metadata.registration_endpoint
    );

    // Bind a one-shot localhost callback listener. We bind first so that the
    // chosen port is part of the redirect_uri we register / authorize with.
    let listener = bind_callback_listener()?;
    let port = listener.local_addr()?.port();
    let redirect_uri: url::Url = format!("http://127.0.0.1:{port}/callback")
        .parse()
        .expect("constructed redirect URI is valid");

    // Try RFC 7591 dynamic client registration if the server supports it;
    // fall back to prompting the user for a pre-registered client_id.
    let (client_id, client_secret) =
        obtain_client_credentials(&metadata, server_name, &redirect_uri).await?;

    let scopes = pick_scopes(&metadata);
    let pkce = Pkce::generate()?;
    let state = oauth2::random_url_safe(16)?;
    let authorize_url = oauth2::build_authorize_url(
        &metadata,
        &client_id,
        &redirect_uri,
        &scopes,
        &state,
        &pkce,
        Some(server_url),
    );

    info!(
        "{}",
        fmt_cyan_bold(&format!(
            "Opening browser to authorize pctx with {server_name}..."
        ))
    );
    info!(
        "{}",
        fmt_dimmed(&format!(
            "If the browser does not open automatically, visit:\n  {authorize_url}"
        ))
    );
    if let Err(e) = webbrowser::open(authorize_url.as_str()) {
        warn!("Failed to launch browser automatically: {e}");
    }

    // Block until the redirect arrives (or we time out).
    let callback = wait_for_callback(listener, &state)?;
    info!("{}", fmt_good_check("Authorization code received"));

    let bundle = oauth2::exchange_code(
        &metadata,
        &client_id,
        client_secret.as_deref(),
        &callback.code,
        &pkce.verifier,
        &redirect_uri,
        Some(server_url),
    )
    .await
    .context("Failed to exchange authorization code for tokens")?;

    let token_ref = AuthConfig::default_oauth_token_ref(server_name);
    let token_ref_resolved = token_ref
        .resolve()
        .await
        .context("Failed to resolve OAuth token_ref")?;
    bundle
        .save(&token_ref_resolved)
        .context("Failed to persist OAuth token bundle to keychain")?;
    info!(
        "{}",
        fmt_good_check(&format!(
            "OAuth tokens stored in system keychain ({token_ref_resolved})"
        ))
    );

    Ok(AuthConfig::OAuth { token_ref, scopes })
}

/// Bind a localhost TCP listener on an OS-assigned port. We use the standard
/// blocking listener (consumed once on the calling thread) because the OAuth
/// callback happens exactly once per flow and we want zero async runtime
/// dependencies for `tiny_http`.
fn bind_callback_listener() -> Result<StdTcpListener> {
    // Bind to 127.0.0.1:0 — the OS picks a free ephemeral port. The localhost
    // requirement matches what most OAuth servers will allow without explicit
    // pre-registration when DCR isn't available.
    let listener = StdTcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .context("Failed to bind localhost callback listener")?;
    listener
        .set_nonblocking(false)
        .context("Failed to set listener to blocking mode")?;
    Ok(listener)
}

struct CallbackResult {
    code: String,
}

/// Wait for the OAuth provider to redirect the user back to our localhost
/// listener. Validates the `state` parameter and returns the authorization
/// `code`. Times out after 5 minutes.
fn wait_for_callback(listener: StdTcpListener, expected_state: &str) -> Result<CallbackResult> {
    let server = tiny_http::Server::from_listener(listener, None)
        .map_err(|e| anyhow::anyhow!("Failed to start callback HTTP server: {e}"))?;

    let timeout = Duration::from_secs(300);
    let request = server
        .recv_timeout(timeout)
        .map_err(|e| anyhow::anyhow!("Callback listener error: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("Timed out waiting for OAuth redirect after 5 minutes"))?;

    // Parse query string from request URL like "/callback?code=...&state=..."
    let url_str = format!("http://127.0.0.1{}", request.url());
    let parsed = url::Url::parse(&url_str)
        .with_context(|| format!("Invalid callback URL from browser: {}", request.url()))?;
    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut error: Option<String> = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            _ => {}
        }
    }

    let html_body = |success: bool, message: &str| -> String {
        let title = if success {
            "pctx: success"
        } else {
            "pctx: error"
        };
        format!(
            "<!doctype html><html><head><meta charset=utf-8><title>{title}</title>\
             <style>body{{font-family:system-ui,sans-serif;padding:3em;max-width:32em;margin:auto}}\
             h1{{color:{color}}}</style></head><body>\
             <h1>{title}</h1><p>{message}</p>\
             <p>You can close this tab and return to your terminal.</p></body></html>",
            color = if success { "#0a7" } else { "#c33" }
        )
    };

    if let Some(err) = error {
        let body = html_body(false, &format!("Authorization failed: {err}"));
        let _ = request.respond(html_response(&body));
        anyhow::bail!("OAuth provider returned error: {err}");
    }

    let state = state.context("OAuth callback missing 'state' parameter")?;
    if state != expected_state {
        let body = html_body(false, "State mismatch — possible CSRF attempt.");
        let _ = request.respond(html_response(&body));
        anyhow::bail!("OAuth state mismatch: expected '{expected_state}', got '{state}'");
    }

    let code = code.context("OAuth callback missing 'code' parameter")?;

    let body = html_body(
        true,
        "Authorization successful. pctx has stored your tokens securely.",
    );
    let _ = request.respond(html_response(&body));

    Ok(CallbackResult { code })
}

fn html_response(body: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let bytes = body.as_bytes().to_vec();
    let len = bytes.len();
    tiny_http::Response::new(
        tiny_http::StatusCode(200),
        vec![
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                .expect("static header is valid"),
        ],
        std::io::Cursor::new(bytes),
        Some(len),
        None,
    )
}

/// Either dynamically register a new OAuth client (RFC 7591) or fall back to
/// prompting the user for a pre-registered `client_id` / `client_secret`.
async fn obtain_client_credentials(
    metadata: &OAuthMetadata,
    server_name: &str,
    redirect_uri: &url::Url,
) -> Result<(String, Option<String>)> {
    if let Some(reg) = &metadata.registration_endpoint {
        debug!("Attempting RFC 7591 dynamic client registration at {reg}");
        let scopes = pick_scopes(metadata);
        match oauth2::dynamic_register(
            reg,
            &[redirect_uri.to_string()],
            &format!("pctx ({server_name})"),
            &scopes,
        )
        .await
        {
            Ok((id, secret)) => {
                info!(
                    "{}",
                    fmt_good_check("Dynamically registered OAuth client with upstream server")
                );
                return Ok((id, secret));
            }
            Err(e) => {
                warn!("Dynamic client registration failed: {e}; falling back to manual entry");
            }
        }
    }

    // Manual fallback — prompt the user.
    info!(
        "{}",
        fmt_dimmed(
            "This server does not support dynamic client registration. \
             Enter the OAuth client_id you have pre-registered with the server."
        )
    );
    info!(
        "{}",
        fmt_dimmed(&format!("Use redirect URI: {redirect_uri}"))
    );
    let client_id = inquire::Text::new("OAuth client_id:")
        .with_validator(inquire::min_length!(
            1,
            "client_id must be at least 1 character"
        ))
        .prompt()?;
    let client_secret = inquire::Text::new("OAuth client_secret (leave blank for public client):")
        .prompt()
        .ok()
        .filter(|s| !s.is_empty());
    Ok((client_id, client_secret))
}
