//! OAuth 2.1 / RFC 8414 / RFC 9728 / RFC 7591 support for upstream MCP servers.
//!
//! This module provides the protocol building blocks (discovery, PKCE, token
//! exchange, refresh, and keychain-backed token storage) used by the CLI to
//! drive an interactive browser-based authorization flow and by
//! [`crate::server::ServerConfig::connect`] to apply tokens at request time.
//!
//! The interactive browser/callback orchestration lives in the CLI crate
//! (`pctx::utils::oauth_flow`) — this module deliberately stays
//! transport-agnostic so it can be reused outside the CLI.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::debug;

/// Authorization-server metadata as defined by RFC 8414 / `OpenID` Connect
/// Discovery. Only the fields pctx actually uses are kept.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthMetadata {
    pub issuer: Option<String>,
    pub authorization_endpoint: url::Url,
    pub token_endpoint: url::Url,
    #[serde(default)]
    pub registration_endpoint: Option<url::Url>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
}

/// RFC 9728 protected-resource metadata. The MCP server publishes this at
/// `<resource>/.well-known/oauth-protected-resource` to point clients at the
/// authorization server(s) it trusts.
#[derive(Debug, Clone, Deserialize)]
struct ProtectedResourceMetadata {
    #[serde(default)]
    authorization_servers: Vec<url::Url>,
}

/// Persistent OAuth credentials for a single upstream server. Stored as JSON
/// in the system keychain under a single key — never written to `pctx.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBundle {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    /// Unix-epoch seconds when the access token expires. `0` means unknown
    /// (assume valid forever / refresh on 401).
    #[serde(default)]
    pub expires_at: u64,
    pub token_endpoint: url::Url,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

fn default_token_type() -> String {
    "Bearer".into()
}

impl TokenBundle {
    /// Refresh-skew: refresh proactively if token expires within this many
    /// seconds.
    pub const REFRESH_SKEW_SECS: u64 = 60;

    pub fn is_expired(&self) -> bool {
        if self.expires_at == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now + Self::REFRESH_SKEW_SECS >= self.expires_at
    }

    /// Load a token bundle from the system keychain.
    ///
    /// # Errors
    /// Returns an error if no entry exists for `token_ref`, the keychain is
    /// inaccessible, or the stored value cannot be deserialized.
    pub fn load(token_ref: &str) -> Result<Self> {
        let entry = keyring::Entry::new("pctx", token_ref)
            .context("Failed to create keychain entry for OAuth token")?;
        let json = entry
            .get_password()
            .with_context(|| format!("No OAuth token in keychain for '{token_ref}'"))?;
        serde_json::from_str(&json).context("Failed to parse OAuth token bundle from keychain")
    }

    /// Save (or overwrite) the token bundle in the system keychain.
    ///
    /// # Errors
    /// Returns an error if the bundle cannot be serialized or the keychain
    /// rejects the write.
    pub fn save(&self, token_ref: &str) -> Result<()> {
        let entry = keyring::Entry::new("pctx", token_ref)
            .context("Failed to create keychain entry for OAuth token")?;
        let json = serde_json::to_string(self).context("Failed to serialize OAuth token bundle")?;
        entry
            .set_password(&json)
            .context("Failed to store OAuth token bundle in keychain")?;
        debug!("OAuth token bundle saved to keychain ref={token_ref}");
        Ok(())
    }

    /// Delete the token bundle from the keychain. No-op if not present.
    ///
    /// # Errors
    /// Returns an error only on unexpected keychain failures (a missing
    /// entry is treated as success).
    pub fn delete(token_ref: &str) -> Result<()> {
        let entry = keyring::Entry::new("pctx", token_ref)
            .context("Failed to create keychain entry for OAuth token")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    }
}

/// Try to discover OAuth metadata for an MCP server URL.
///
/// Tries (in order):
/// 1. RFC 9728 protected-resource metadata at
///    `<origin>/.well-known/oauth-protected-resource` — if present, follow
///    its `authorization_servers[0]` and recurse.
/// 2. RFC 8414 authorization-server metadata at
///    `<origin>/.well-known/oauth-authorization-server`.
/// 3. `OpenID` Connect discovery at `<origin>/.well-known/openid-configuration`.
///
/// Returns `Ok(None)` if none of those return parseable JSON — i.e. the
/// server is not OAuth-protected.
///
/// # Errors
/// Returns an error only on unexpected transport failures (DNS, TLS). A
/// 404 / non-JSON response is treated as "not OAuth" and returns `Ok(None)`.
pub async fn discover(server_url: &url::Url) -> Result<Option<OAuthMetadata>> {
    let client = reqwest::Client::builder()
        .build()
        .context("Failed to build HTTP client for OAuth discovery")?;

    // 1. RFC 9728 — protected resource metadata
    let pr_url = well_known(server_url, "oauth-protected-resource");
    if let Some(meta) = fetch_json::<ProtectedResourceMetadata>(&client, &pr_url).await?
        && let Some(auth_server) = meta.authorization_servers.first()
    {
        debug!("Discovered protected-resource metadata pointing at {auth_server}");
        if let Some(m) = discover_auth_server(&client, auth_server).await? {
            return Ok(Some(m));
        }
    }

    // 2/3. Try the resource origin itself as the auth server.
    discover_auth_server(&client, server_url).await
}

async fn discover_auth_server(
    client: &reqwest::Client,
    base: &url::Url,
) -> Result<Option<OAuthMetadata>> {
    let as_url = well_known(base, "oauth-authorization-server");
    if let Some(meta) = fetch_json::<OAuthMetadata>(client, &as_url).await? {
        return Ok(Some(meta));
    }
    let oidc_url = well_known(base, "openid-configuration");
    fetch_json::<OAuthMetadata>(client, &oidc_url).await
}

/// Build a `<scheme>://<host>/.well-known/<name>` URL preserving the origin
/// of `base` and discarding any path/query.
fn well_known(base: &url::Url, name: &str) -> url::Url {
    let mut u = base.clone();
    u.set_path(&format!("/.well-known/{name}"));
    u.set_query(None);
    u.set_fragment(None);
    u
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &url::Url,
) -> Result<Option<T>> {
    debug!("OAuth discovery: GET {url}");
    let resp = match client.get(url.clone()).send().await {
        Ok(r) => r,
        Err(e) => {
            debug!("Discovery fetch failed for {url}: {e}");
            return Ok(None);
        }
    };
    if !resp.status().is_success() {
        return Ok(None);
    }
    match resp.json::<T>().await {
        Ok(t) => Ok(Some(t)),
        Err(e) => {
            debug!("Discovery body parse failed for {url}: {e}");
            Ok(None)
        }
    }
}

// === RFC 7591 Dynamic Client Registration ===========================

#[derive(Debug, Serialize)]
struct DcrRequest<'a> {
    redirect_uris: &'a [String],
    client_name: &'a str,
    grant_types: [&'a str; 2],
    response_types: [&'a str; 1],
    token_endpoint_auth_method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DcrResponse {
    client_id: String,
    #[serde(default)]
    client_secret: Option<String>,
}

/// Best-effort RFC 7591 dynamic client registration. Returns `(client_id,
/// client_secret)` on success.
///
/// # Errors
/// Returns an error only on transport / non-success responses. Callers should
/// be prepared to fall back to prompting the user for a pre-registered
/// `client_id` when this fails.
pub async fn dynamic_register(
    registration_endpoint: &url::Url,
    redirect_uris: &[String],
    client_name: &str,
    scopes: &[String],
) -> Result<(String, Option<String>)> {
    let body = DcrRequest {
        redirect_uris,
        client_name,
        grant_types: ["authorization_code", "refresh_token"],
        response_types: ["code"],
        // Public client (no client secret); we'll switch to client_secret_basic
        // automatically if the server returns one.
        token_endpoint_auth_method: "none",
        scope: if scopes.is_empty() {
            None
        } else {
            Some(scopes.join(" "))
        },
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(registration_endpoint.clone())
        .json(&body)
        .send()
        .await
        .context("Dynamic client registration request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Dynamic client registration failed: {status}: {text}");
    }

    let parsed: DcrResponse = resp
        .json()
        .await
        .context("Dynamic client registration returned invalid JSON")?;
    Ok((parsed.client_id, parsed.client_secret))
}

// === PKCE helpers =====================================================

/// A freshly generated PKCE code verifier + challenge pair.
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    /// Generate a fresh PKCE pair using SHA-256 (S256).
    ///
    /// # Errors
    /// Returns an error if the OS RNG is unavailable.
    pub fn generate() -> Result<Self> {
        let verifier = random_url_safe(32)?;
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(digest);
        Ok(Self {
            verifier,
            challenge,
        })
    }
}

/// Generate `n` random bytes and return them base64url-encoded (no padding).
/// Suitable for OAuth `state` and PKCE `code_verifier`.
///
/// # Errors
/// Returns an error if the OS RNG is unavailable.
pub fn random_url_safe(n: usize) -> Result<String> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| anyhow::anyhow!("OS RNG unavailable: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(&buf))
}

/// URL-encode a list of form pairs (`application/x-www-form-urlencoded`).
/// We do this by hand because the `reqwest` `RequestBuilder::form` helper
/// isn't available with the feature set `pctx_config` enables.
fn url_encoded_body(pairs: &[(&str, &str)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        s.append_pair(k, v);
    }
    s.finish()
}

const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";

/// Build the authorization-endpoint URL the user's browser should be sent to.
pub fn build_authorize_url(
    metadata: &OAuthMetadata,
    client_id: &str,
    redirect_uri: &url::Url,
    scopes: &[String],
    state: &str,
    pkce: &Pkce,
    resource: Option<&url::Url>,
) -> url::Url {
    let mut u = metadata.authorization_endpoint.clone();
    {
        let mut q = u.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", client_id);
        q.append_pair("redirect_uri", redirect_uri.as_str());
        q.append_pair("state", state);
        q.append_pair("code_challenge", &pkce.challenge);
        q.append_pair("code_challenge_method", "S256");
        if !scopes.is_empty() {
            q.append_pair("scope", &scopes.join(" "));
        }
        if let Some(res) = resource {
            // RFC 8707 resource indicator
            q.append_pair("resource", res.as_str());
        }
    }
    u
}

// === Token endpoint exchanges =========================================

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    token_type: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn token_response_into_bundle(
    resp: TokenResponse,
    token_endpoint: url::Url,
    client_id: String,
    client_secret: Option<String>,
    fallback_refresh: Option<String>,
) -> TokenBundle {
    let expires_at = resp.expires_in.map_or(0, |s| now_secs() + s);
    TokenBundle {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token.or(fallback_refresh),
        token_type: resp.token_type,
        expires_at,
        token_endpoint,
        client_id,
        client_secret,
    }
}

/// Exchange an authorization code (returned via the redirect URI) for tokens.
///
/// # Errors
/// Returns an error if the token endpoint is unreachable, returns a
/// non-success status, or returns a body that does not parse as a token
/// response.
pub async fn exchange_code(
    metadata: &OAuthMetadata,
    client_id: &str,
    client_secret: Option<&str>,
    code: &str,
    code_verifier: &str,
    redirect_uri: &url::Url,
    resource: Option<&url::Url>,
) -> Result<TokenBundle> {
    let redirect_uri_str = redirect_uri.to_string();
    let resource_str = resource.map(url::Url::to_string);
    let mut pairs: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri_str.as_str()),
        ("client_id", client_id),
        ("code_verifier", code_verifier),
    ];
    if let Some(res) = resource_str.as_deref() {
        pairs.push(("resource", res));
    }
    let body = url_encoded_body(&pairs);

    let client = reqwest::Client::new();
    let mut req = client
        .post(metadata.token_endpoint.clone())
        .header(http::header::CONTENT_TYPE, FORM_CONTENT_TYPE)
        .body(body);
    if let Some(secret) = client_secret {
        req = req.basic_auth(client_id, Some(secret));
    }
    let resp = req
        .send()
        .await
        .context("OAuth token exchange request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OAuth token exchange failed: {status}: {text}");
    }

    let parsed: TokenResponse = resp
        .json()
        .await
        .context("OAuth token endpoint returned invalid JSON")?;
    Ok(token_response_into_bundle(
        parsed,
        metadata.token_endpoint.clone(),
        client_id.into(),
        client_secret.map(str::to_string),
        None,
    ))
}

/// Use a refresh token to obtain a fresh access (and possibly refresh) token.
///
/// # Errors
/// Returns an error if the bundle has no refresh token, the token endpoint
/// is unreachable, or it returns a non-success response.
pub async fn refresh(bundle: &TokenBundle) -> Result<TokenBundle> {
    let refresh_token = bundle
        .refresh_token
        .as_deref()
        .context("OAuth bundle has no refresh token; re-run `pctx mcp add` to re-authorize")?;

    let pairs = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", bundle.client_id.as_str()),
    ];
    let body = url_encoded_body(&pairs);

    let client = reqwest::Client::new();
    let mut req = client
        .post(bundle.token_endpoint.clone())
        .header(http::header::CONTENT_TYPE, FORM_CONTENT_TYPE)
        .body(body);
    if let Some(secret) = bundle.client_secret.as_deref() {
        req = req.basic_auth(&bundle.client_id, Some(secret));
    }
    let resp = req.send().await.context("OAuth refresh request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OAuth refresh failed: {status}: {text}");
    }

    let parsed: TokenResponse = resp
        .json()
        .await
        .context("OAuth refresh returned invalid JSON")?;
    Ok(token_response_into_bundle(
        parsed,
        bundle.token_endpoint.clone(),
        bundle.client_id.clone(),
        bundle.client_secret.clone(),
        // Some servers don't return a new refresh token — keep the old one.
        bundle.refresh_token.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_pair_is_valid_s256() {
        let p = Pkce::generate().unwrap();
        // verifier base64url-encoded 32 bytes => 43 chars
        assert_eq!(p.verifier.len(), 43);
        // challenge is sha256(verifier) base64url
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(p.verifier.as_bytes()));
        assert_eq!(p.challenge, expected);
    }

    #[test]
    fn well_known_strips_path_and_query() {
        let base: url::Url = "https://mcp.example.com/sse?token=foo".parse().unwrap();
        let w = well_known(&base, "oauth-authorization-server");
        assert_eq!(
            w.as_str(),
            "https://mcp.example.com/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn token_bundle_round_trip() {
        let bundle = TokenBundle {
            access_token: "at".into(),
            refresh_token: Some("rt".into()),
            token_type: "Bearer".into(),
            expires_at: 1_700_000_000,
            token_endpoint: "https://issuer.example.com/token".parse().unwrap(),
            client_id: "client123".into(),
            client_secret: None,
        };
        let json = serde_json::to_string(&bundle).unwrap();
        let parsed: TokenBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "at");
        assert_eq!(parsed.client_id, "client123");
        assert_eq!(parsed.expires_at, 1_700_000_000);
    }

    #[test]
    fn token_bundle_expiry() {
        let bundle = TokenBundle {
            access_token: "at".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_at: 1, // ancient
            token_endpoint: "https://x/token".parse().unwrap(),
            client_id: "c".into(),
            client_secret: None,
        };
        assert!(bundle.is_expired());

        let unknown = TokenBundle {
            expires_at: 0,
            ..bundle.clone()
        };
        assert!(!unknown.is_expired());

        let future = TokenBundle {
            expires_at: now_secs() + 3600,
            ..bundle
        };
        assert!(!future.is_expired());
    }

    #[test]
    fn build_authorize_url_includes_pkce_and_state() {
        let metadata = OAuthMetadata {
            issuer: None,
            authorization_endpoint: "https://issuer/authorize".parse().unwrap(),
            token_endpoint: "https://issuer/token".parse().unwrap(),
            registration_endpoint: None,
            scopes_supported: vec![],
            code_challenge_methods_supported: vec!["S256".into()],
        };
        let pkce = Pkce::generate().unwrap();
        let url = build_authorize_url(
            &metadata,
            "myclient",
            &"http://127.0.0.1:8765/callback".parse().unwrap(),
            &["read".into(), "write".into()],
            "abc-state",
            &pkce,
            Some(&"https://mcp.example.com".parse().unwrap()),
        );
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(q.get("client_id").map(String::as_str), Some("myclient"));
        assert_eq!(
            q.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(q.get("code_challenge"), Some(&pkce.challenge));
        assert_eq!(q.get("scope").map(String::as_str), Some("read write"));
        assert_eq!(q.get("state").map(String::as_str), Some("abc-state"));
        assert_eq!(
            q.get("resource").map(String::as_str),
            Some("https://mcp.example.com/")
        );
    }
}
