use log::debug;
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        ClientCapabilities, ClientInfo, Implementation, InitializeRequestParam, ProtocolVersion,
    },
    service::{ClientInitializeError, RunningService},
    transport::{StreamableHttpClientTransport, streamable_http_client::StreamableHttpError},
};

/// Error types for MCP server connection failures
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum InitMCPClientError {
    /// Server requires authentication (401 Unauthorized)
    #[error("Server requires OAuth authentication")]
    RequiresOAuth,
    /// Server requires authentication (401 Unauthorized)
    #[error("Server requires authentication")]
    RequiresAuth,
    /// Connection failed (network error, invalid URL, etc.)
    #[error("Failed to connect: {0}")]
    Failed(String),
}

pub(crate) async fn init_mcp_client(
    url: &str,
) -> Result<RunningService<RoleClient, InitializeRequestParam>, InitMCPClientError> {
    let transport = StreamableHttpClientTransport::from_uri(url);
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
        Err(ClientInitializeError::TransportError { error, context }) => {
            if let Some(s_err) = error
                .error
                .downcast_ref::<StreamableHttpError<reqwest::Error>>()
                && let StreamableHttpError::AuthRequired(a_err) = s_err
            {
                debug!("Server (`{url}`) requires auth, testing OAuth 2.1 support...");
                debug!(
                    "www_authenticate_header: {}",
                    &a_err.www_authenticate_header
                );
                if let Ok(_oauth_state) = rmcp::transport::auth::OAuthState::new(url, None).await {
                    debug!("Server supports OAuth 2.1");
                    return Err(InitMCPClientError::RequiresOAuth);
                }
                return Err(InitMCPClientError::RequiresAuth);
            }
            Err(InitMCPClientError::Failed(format!(
                "Failed initialize request with MCP server ({url}): {error} {context} "
            )))
        }
        Err(e) => Err(InitMCPClientError::Failed(format!(
            "Failed initialize request with MCP server ({url}): {e} "
        ))),
    }
}
