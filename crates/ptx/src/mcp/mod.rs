pub(crate) mod auth;
pub(crate) mod config;
pub(crate) mod deno_pool;
pub(crate) mod inspect;
pub(crate) mod tools;
pub(crate) mod upstream;

use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
};

use crate::mcp::{
    deno_pool::DenoExecutor,
    tools::{PtxTools, UpstreamMcp},
};

pub(crate) struct PtxMcp;
impl PtxMcp {
    pub(crate) async fn serve(host: &str, port: u16, mcp: UpstreamMcp) {
        let allowed_hosts = vec![mcp.url.clone()];
        let executor = DenoExecutor::new(Some(allowed_hosts.clone()));
        log::info!("Starting sandbox with access to host: {allowed_hosts:?}...");

        let service = StreamableHttpService::new(
            // || Ok(counter::Counter::new()),
            move || Ok(PtxTools::with_executor(executor.clone()).register_mcp(mcp.clone())),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig {
                stateful_mode: false,
                ..Default::default()
            },
        );

        let router = axum::Router::new().nest_service("/mcp", service);
        let tcp_listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
            .await
            .unwrap();
        log::info!("Listening on {host}:{port}...");
        let _ = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
            .await;
    }

    pub(crate) async fn serve_multi(host: &str, port: u16, mcps: Vec<UpstreamMcp>) {
        let allowed_hosts: Vec<String> = mcps.iter().map(|m| m.url.clone()).collect();
        let executor = DenoExecutor::new(Some(allowed_hosts.clone()));
        log::info!("Starting sandbox with access to host: {allowed_hosts:?}...");

        let service = StreamableHttpService::new(
            move || {
                let mut tools = PtxTools::with_executor(executor.clone());
                for mcp in mcps.clone() {
                    tools = tools.register_mcp(mcp);
                }
                Ok(tools)
            },
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig {
                stateful_mode: false,
                ..Default::default()
            },
        );

        let router = axum::Router::new().nest_service("/mcp", service);
        let tcp_listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
            .await
            .unwrap();
        log::info!("Listening on {host}:{port}...");
        let _ = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
            .await;
    }
}
