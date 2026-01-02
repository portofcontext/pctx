use http::{HeaderMap, HeaderName, HeaderValue};
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        ClientCapabilities, ClientInfo, Implementation, InitializeRequestParam, ProtocolVersion,
    },
    service::{ClientInitializeError, RunningService},
    transport::{
        StreamableHttpClientTransport,
        child_process::{ConfigureCommandExt, TokioChildProcess},
        streamable_http_client::{StreamableHttpClientTransportConfig, StreamableHttpError},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;
use tokio::process::Command;

pub use rmcp::ServiceError;

use super::auth::AuthConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    #[serde(flatten)]
    pub transport: ServerTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerTransport {
    Http(HttpServerConfig),
    Stdio(StdioServerConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpServerConfig {
    pub url: url::Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StdioServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

impl ServerConfig {
    pub fn new(name: String, url: url::Url) -> Self {
        Self {
            name,
            transport: ServerTransport::Http(HttpServerConfig { url, auth: None }),
        }
    }

    pub fn new_stdio(
        name: String,
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    ) -> Self {
        Self {
            name,
            transport: ServerTransport::Stdio(StdioServerConfig { command, args, env }),
        }
    }

    pub fn http(&self) -> Option<&HttpServerConfig> {
        match &self.transport {
            ServerTransport::Http(cfg) => Some(cfg),
            ServerTransport::Stdio(_) => None,
        }
    }

    pub fn http_mut(&mut self) -> Option<&mut HttpServerConfig> {
        match &mut self.transport {
            ServerTransport::Http(cfg) => Some(cfg),
            ServerTransport::Stdio(_) => None,
        }
    }

    pub fn stdio(&self) -> Option<&StdioServerConfig> {
        match &self.transport {
            ServerTransport::Stdio(cfg) => Some(cfg),
            ServerTransport::Http(_) => None,
        }
    }

    pub fn set_auth(&mut self, auth: Option<AuthConfig>) {
        if let Some(http_cfg) = self.http_mut() {
            http_cfg.auth = auth;
        }
    }

    pub fn display_target(&self) -> String {
        match &self.transport {
            ServerTransport::Http(cfg) => cfg.url.to_string(),
            ServerTransport::Stdio(cfg) => {
                if cfg.args.is_empty() {
                    cfg.command.clone()
                } else {
                    format!("{} {}", cfg.command, cfg.args.join(" "))
                }
            }
        }
    }

    /// Connects to the MCP server as specified in the `ServerConfig`
    ///
    /// # Errors
    ///
    /// This function will return an error if unable to connect and send the
    /// initialization request
    pub async fn connect(
        &self,
    ) -> Result<RunningService<RoleClient, InitializeRequestParam>, McpConnectionError> {
        let init_request = ClientInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "pctx-client".to_string(),
                version: option_env!("CARGO_PKG_VERSION")
                    .unwrap_or("0.1.0")
                    .to_string(),
                ..Default::default()
            },
        };

        match &self.transport {
            ServerTransport::Http(http_cfg) => {
                let mut default_headers = HeaderMap::new();

                // Add auth to http client
                if let Some(a) = &http_cfg.auth {
                    match a {
                        AuthConfig::Bearer { token } => {
                            let resolved = token
                                .resolve()
                                .await
                                .map_err(|e| McpConnectionError::Failed(e.to_string()))?;
                            default_headers.append(
                                http::header::AUTHORIZATION,
                                HeaderValue::from_str(&format!("Bearer {resolved}"))
                                    .map_err(|e| McpConnectionError::Failed(e.to_string()))?,
                            );
                        }
                        AuthConfig::Headers { headers } => {
                            for (name, val) in headers {
                                let resolved = val
                                    .resolve()
                                    .await
                                    .map_err(|e| McpConnectionError::Failed(e.to_string()))?;
                                default_headers.append(
                                    HeaderName::from_str(name)
                                        .map_err(|e| McpConnectionError::Failed(e.to_string()))?,
                                    HeaderValue::from_str(&resolved)
                                        .map_err(|e| McpConnectionError::Failed(e.to_string()))?,
                                );
                            }
                        }
                    }
                }

                let reqwest_client = reqwest::Client::builder()
                    .default_headers(default_headers)
                    .build()
                    .map_err(|e| McpConnectionError::Failed(e.to_string()))?;

                let transport = StreamableHttpClientTransport::with_client(
                    reqwest_client,
                    StreamableHttpClientTransportConfig {
                        uri: http_cfg.url.as_str().into(),
                        ..Default::default()
                    },
                );
                match init_request.serve(transport).await {
                    Ok(c) => Ok(c),
                    Err(ClientInitializeError::TransportError { error, .. }) => {
                        if let Some(s_err) = error
                            .error
                            .downcast_ref::<StreamableHttpError<reqwest::Error>>()
                            && let StreamableHttpError::AuthRequired(_) = s_err
                        {
                            return Err(McpConnectionError::RequiresAuth);
                        }
                        Err(McpConnectionError::Failed(error.error.to_string()))
                    }
                    Err(e) => Err(McpConnectionError::Failed(format!("{e}"))),
                }
            }
            ServerTransport::Stdio(stdio_cfg) => {
                // Parse the command using shell-style parsing if it contains spaces and no explicit args
                let (cmd, args) = if stdio_cfg.args.is_empty() && stdio_cfg.command.contains(' ') {
                    // Parse the command string using shell-style parsing
                    let parts = shlex::split(&stdio_cfg.command).ok_or_else(|| {
                        McpConnectionError::Failed(format!(
                            "Failed to parse command: {}",
                            stdio_cfg.command
                        ))
                    })?;

                    if parts.is_empty() {
                        return Err(McpConnectionError::Failed("Empty command".to_string()));
                    }

                    (parts[0].clone(), parts[1..].to_vec())
                } else {
                    // Use command and args as-is
                    (stdio_cfg.command.clone(), stdio_cfg.args.clone())
                };

                let transport =
                    TokioChildProcess::new(Command::new(&cmd).configure(|cmd_builder| {
                        cmd_builder.args(&args);
                        if !stdio_cfg.env.is_empty() {
                            cmd_builder.envs(&stdio_cfg.env);
                        }
                    }))
                    .map_err(|e| McpConnectionError::Failed(e.to_string()))?;

                init_request
                    .serve(transport)
                    .await
                    .map_err(|e| McpConnectionError::Failed(format!("{e}")))
            }
        }
    }
}

/// Simplified error types for MCP server connection failures
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpConnectionError {
    /// Server requires authentication
    #[error("Server requires authentication")]
    RequiresAuth,
    /// Connection failed (network error, invalid URL, etc.)
    #[error("Failed to connect: {0}")]
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::ServerConfig;
    use serde_json::json;

    #[test]
    fn test_deserialize_http_server_config() {
        let payload = json!({
            "name": "http",
            "url": "http://localhost:8080/mcp"
        });
        let cfg: ServerConfig = serde_json::from_value(payload).unwrap();
        let http = cfg.http().expect("expected http config");
        assert_eq!(http.url.as_str(), "http://localhost:8080/mcp");
    }

    #[test]
    fn test_deserialize_stdio_server_config() {
        let payload = json!({
            "name": "stdio",
            "command": "node",
            "args": ["./server.js"],
            "env": {
                "NODE_ENV": "development"
            }
        });
        let cfg: ServerConfig = serde_json::from_value(payload).unwrap();
        let stdio = cfg.stdio().expect("expected stdio config");
        assert_eq!(stdio.command, "node");
        assert_eq!(stdio.args, vec!["./server.js"]);
        assert_eq!(
            stdio.env.get("NODE_ENV").map(String::as_str),
            Some("development")
        );
    }
}
