use anyhow::Result;
use log::info;
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};

use crate::mcp::{auth::get_server_credentials, config::Config};

enum ConnectionStatus {
    Success,
    Failed(String),
}

async fn test_connection(server: &crate::mcp::config::ServerConfig) -> ConnectionStatus {
    // Get authentication credentials if configured
    let credentials = match get_server_credentials(server).await {
        Ok(creds) => creds,
        Err(e) => return ConnectionStatus::Failed(format!("Auth error: {e}")),
    };

    // Build the URL with query params if needed
    let mut url = server.url.clone();
    if let Some(creds) = &credentials
        && !creds.query.is_empty()
    {
        let query_string: Vec<String> = creds
            .query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        url = format!("{}?{}", url, query_string.join("&"));
    }

    let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url);

    if let Some(creds) = &credentials
        && let Some(auth_value) = creds.headers.get("Authorization")
    {
        let token = auth_value.strip_prefix("Bearer ").unwrap_or(auth_value);
        transport_config = transport_config.auth_header(token);
    }

    let transport = StreamableHttpClientTransport::from_config(transport_config);

    // Try to initialize MCP connection
    match ().serve(transport).await {
        Ok(_client) => ConnectionStatus::Success,
        Err(e) => ConnectionStatus::Failed(format!("{e}")),
    }
}

pub(crate) async fn handle() -> Result<()> {
    let config = Config::load()?;

    if config.servers.is_empty() {
        info!("No MCP servers configured.");
        info!("");
        info!("Add a server with: pctl mcp add <name> <url>");
        return Ok(());
    }

    info!("Checking MCP server health...");
    info!("");

    // Test all servers
    for server in &config.servers {
        let status = test_connection(server).await;

        let protocol = if server.url.starts_with("https://") {
            "HTTPS"
        } else {
            "HTTP"
        };

        match status {
            ConnectionStatus::Success => {
                info!(
                    "{}: {} ({}) - ✓ Connected",
                    server.name, server.url, protocol
                );
            }
            ConnectionStatus::Failed(reason) => {
                info!(
                    "{}: {} ({}) - ✗ {}",
                    server.name, server.url, protocol, reason
                );
            }
        }
    }

    Ok(())
}
