use http::{HeaderMap, HeaderName, HeaderValue};
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        ClientCapabilities, ClientInfo, Implementation, InitializeRequestParam, ProtocolVersion,
    },
    service::{ClientInitializeError, RunningService},
    transport::{
        StreamableHttpClientTransport,
        streamable_http_client::{StreamableHttpClientTransportConfig, StreamableHttpError},
    },
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::auth::AuthConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub url: url::Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
}

impl ServerConfig {
    pub fn new(name: String, url: url::Url) -> Self {
        Self {
            name,
            url,
            auth: None,
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
        let mut default_headers = HeaderMap::new();

        // Add auth to http client
        if let Some(a) = &self.auth {
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
                AuthConfig::Custom { headers } => {
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
                uri: self.url.as_str().into(),
                ..Default::default()
            },
        );
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
