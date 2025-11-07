use anyhow::{Context, Result};
use codegen::case::Case;
use indexmap::IndexMap;
use log::{debug, info};
use reqwest::StatusCode;

use crate::mcp::{client::init_mcp_client, tools::UpstreamTool};

use super::{auth::get_server_credentials, config::ServerConfig, tools::UpstreamMcp};

/// Fetch tools from an upstream MCP server
///
/// This function:
/// 1. Gets authentication credentials for the server (if configured)
/// 2. Makes an HTTP request to the server with auth headers/query params
/// 3. Parses the MCP server's tool list response
/// 4. Returns an ``UpstreamMcp`` instance with the discovered tools
pub(crate) async fn fetch_upstream_tools(server: &ServerConfig) -> Result<UpstreamMcp> {
    info!("Fetching tools from '{}'...", server.name);

    // Get authentication credentials if configured
    let credentials = get_server_credentials(server).await?;

    if credentials.is_some() {
        debug!("Using authentication for '{}'", server.name);
    }

    // TODO: extend init_mcp_client to support auth tokens and use here
    let mcp_client = init_mcp_client(&server.url).await?;

    // Build the HTTP client and request
    // let client = reqwest::Client::new();
    // let mut request = client.get(&server.url);

    // // Add auth headers and query params if available
    // if let Some(creds) = &credentials {
    //     for (key, value) in &creds.headers {
    //         request = request.header(key, value);
    //     }
    //     for (key, value) in &creds.query {
    //         request = request.query(&[(key, value)]);
    //     }
    // }

    // // Make the request
    // let response = request
    //     .send()
    //     .await
    //     .context(format!("Failed to connect to server '{}'", server.name))?;

    // let status = response.status();
    // if !status.is_success() {
    //     anyhow::bail!("Server '{}' returned error status: {}", server.name, status);
    // }

    debug!(
        "Successfully connected to '{}', inspecting tools",
        server.name
    );

    let listed_tools = mcp_client.list_all_tools().await?;
    debug!("Found {} tools", listed_tools.len());

    let mut tools = IndexMap::new();
    for t in listed_tools {
        let tool = UpstreamTool::from_tool(t)?;
        tools.insert(tool.fn_name.clone(), tool);
    }

    let description = mcp_client
        .peer_info()
        .and_then(|p| p.server_info.title.clone())
        .unwrap_or(format!("MCP server at {}", server.url));

    mcp_client.cancel().await?;

    Ok(UpstreamMcp {
        name: server.name.clone(),
        namespace: Case::Pascal.sanitize(&server.name),
        description,
        url: server.url.clone(),
        tools,
    })
}

/// Result of testing connection to an MCP server
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConnectionTestResult {
    /// Successfully connected without authentication
    Success,
    /// Server requires authentication (401 Unauthorized)
    RequiresAuth,
    /// Server returned 403 Forbidden (might need different auth or permissions)
    Forbidden,
    /// OAuth 2.1 is available on this server
    OAuth2Available,
    /// Connection failed (network error, invalid URL, etc.)
    Failed(String),
}

/// Test connection to an MCP server to determine if authentication is needed
///
/// This function:
/// 1. First attempts to connect without authentication using a simple MCP initialize request
/// 2. If the server requires auth (401), checks if it supports OAuth 2.1
/// 3. Returns a result indicating whether auth is needed and what type
pub(crate) async fn test_server_connection(url: &str) -> ConnectionTestResult {
    // Try a simple MCP initialize request without auth
    debug!("Testing connection to {url} without authentication...");

    let client = reqwest::Client::new();

    // Create a minimal MCP initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "ptx",
                "version": "0.1.0"
            }
        }
    });

    match client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&init_request)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            debug!("Server responded with status: {status}");

            if status == StatusCode::OK || status.is_success() {
                ConnectionTestResult::Success
            } else if status == StatusCode::UNAUTHORIZED {
                // Server requires auth - check if it supports OAuth 2.1
                debug!("Server requires auth, testing OAuth 2.1 support...");
                if let Ok(_oauth_state) = rmcp::transport::auth::OAuthState::new(url, None).await {
                    info!("Server supports OAuth 2.1");
                    ConnectionTestResult::OAuth2Available
                } else {
                    ConnectionTestResult::RequiresAuth
                }
            } else if status == StatusCode::FORBIDDEN {
                ConnectionTestResult::Forbidden
            } else if status == StatusCode::METHOD_NOT_ALLOWED {
                // 405 likely means the server is running but expects different format
                // Treat this as success since the server is responding
                debug!("Server returned 405, treating as success (server is responsive)");
                ConnectionTestResult::Success
            } else if status == StatusCode::NOT_ACCEPTABLE {
                // 406 might mean the server doesn't like our headers/format, but it's responding
                // Treat this as success since the server is responsive
                debug!("Server returned 406, treating as success (server is responsive)");
                ConnectionTestResult::Success
            } else {
                ConnectionTestResult::Failed(format!("Server returned status: {status}"))
            }
        }
        Err(e) => {
            debug!("Connection test failed: {e}");
            ConnectionTestResult::Failed(format!("Failed to connect: {e}"))
        }
    }
}
