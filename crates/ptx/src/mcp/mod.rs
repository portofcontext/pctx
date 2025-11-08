pub(crate) mod auth;
pub(crate) mod client;
pub(crate) mod config;
pub(crate) mod token_resolver;
pub(crate) mod tools;
pub(crate) mod upstream;

use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{StreamableHttpService, session::local::LocalSessionManager},
};

use crate::mcp::tools::{PtcxTools, UpstreamMcp};

pub(crate) struct PtcxMcp;
impl PtcxMcp {
    pub(crate) async fn serve(host: &str, port: u16, mcps: Vec<UpstreamMcp>) {
        let allowed_hosts = mcps
            .iter()
            .filter_map(|m| {
                url::Url::parse(&m.url).ok().and_then(|url| {
                    let host = url.host_str()?;
                    if let Some(port) = url.port() {
                        Some(format!("{host}:{port}"))
                    } else {
                        let default_port = if url.scheme() == "https" { 443 } else { 80 };
                        Some(format!("{host}:{default_port}"))
                    }
                })
            })
            .collect::<Vec<_>>();

        log::info!("Starting sandbox with access to host: {allowed_hosts:?}...");

        let service = StreamableHttpService::new(
            move || Ok(PtcxTools::new(allowed_hosts.clone()).with_upstream_mcps(mcps.clone())),
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
