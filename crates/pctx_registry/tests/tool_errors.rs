//! Verifies that error content from upstream MCP tools (`isError: true`) is
//! surfaced in the registry error instead of being dropped.

use std::sync::Arc;

use pctx_config::server::ServerConfig;
use pctx_registry::{PctxRegistry, RegistryError};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{CallToolRequestParams, CallToolResult, Content, ServerCapabilities, ServerInfo},
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::json;

#[derive(Clone)]
struct LedgerServer;

impl ServerHandler for LedgerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    #[allow(clippy::unused_async)]
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(match request.name.as_ref() {
            "text_error" => {
                CallToolResult::error(vec![Content::text("account_id 42 does not exist")])
            }
            "structured_error" => CallToolResult::structured_error(
                json!({"code": "INSUFFICIENT_FUNDS", "retryable": false}),
            ),
            "empty_error" => CallToolResult::error(vec![]),
            _ => CallToolResult::success(vec![Content::text(r#"{"ok":true}"#)]),
        })
    }
}

async fn registry_with_ledger_server() -> PctxRegistry {
    let service = StreamableHttpService::new(
        || Ok(LedgerServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    let registry = PctxRegistry::default();
    registry
        .add_mcp(
            &[
                "text_error".to_string(),
                "structured_error".to_string(),
                "empty_error".to_string(),
                "ok".to_string(),
            ],
            ServerConfig::new(
                "ledger".to_string(),
                url::Url::parse(&format!("http://{addr}/mcp")).unwrap(),
            ),
        )
        .unwrap();
    registry
}

async fn invoke_err(registry: &PctxRegistry, id: &str) -> String {
    match registry.invoke(id, None).await {
        Err(RegistryError::ToolCall(msg)) => msg,
        other => panic!("expected ToolCall error, got {other:?}"),
    }
}

#[tokio::test]
async fn tool_error_text_content_is_surfaced() {
    let registry = registry_with_ledger_server().await;
    assert_eq!(
        invoke_err(&registry, "ledger__text_error").await,
        r#"Tool call "ledger__text_error" failed: account_id 42 does not exist"#
    );
}

#[tokio::test]
async fn tool_error_structured_content_is_surfaced() {
    let registry = registry_with_ledger_server().await;
    assert_eq!(
        invoke_err(&registry, "ledger__structured_error").await,
        r#"Tool call "ledger__structured_error" failed: {"code":"INSUFFICIENT_FUNDS","retryable":false}"#
    );
}

#[tokio::test]
async fn tool_error_without_content_is_generic() {
    let registry = registry_with_ledger_server().await;
    assert_eq!(
        invoke_err(&registry, "ledger__empty_error").await,
        r#"Tool call "ledger__empty_error" failed"#
    );
}

#[tokio::test]
async fn successful_tool_call_is_unchanged() {
    let registry = registry_with_ledger_server().await;
    let val = registry.invoke("ledger__ok", None).await.unwrap();
    assert_eq!(val, json!({"ok": true}));
}
