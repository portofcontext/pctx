use anyhow::Result;
use codegen::case::Case;
use indexmap::IndexMap;
use log::{debug, info};

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
