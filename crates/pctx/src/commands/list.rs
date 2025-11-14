use std::fmt::Display;

use anyhow::Result;
use clap::Parser;
use log::info;
use pctx_config::{
    Config,
    server::{McpConnectionError, ServerConfig},
};
use rmcp::model::InitializeResult;
use url::Url;

use crate::utils::{
    spinner::Spinner,
    styles::{fmt_bold, fmt_cyan, fmt_dimmed, fmt_error, fmt_green, fmt_success},
};

#[derive(Debug, Clone, Parser)]
pub struct ListCmd;

impl ListCmd {
    pub(crate) async fn handle(&self, cfg: Config) -> Result<Config> {
        if cfg.servers.is_empty() {
            info!("No upstream MCP servers configured");
            info!("");
            info!(
                "Run {cmd} to add some to your configuration",
                cmd = fmt_bold("pctx add <NAME> <MCP_URL>")
            );
            return Ok(cfg);
        }

        let num_servers = cfg.servers.len();
        let mut sp = Spinner::new(format!("Listing upstream MCPs... 0/{num_servers}"));
        let mut summaries = vec![];
        for (i, server) in cfg.servers.iter().enumerate() {
            sp.update_text(format!("Listing upstream MCPs... {}/{num_servers}", i + 1));
            summaries.push(UpstreamMcpSummary::new(server).await);
        }

        sp.stop_success("Done");

        for summary in summaries {
            info!("\n{summary}");
        }

        Ok(cfg)
    }
}

struct UpstreamMcpSummary {
    pub url: Url,
    pub name: String,
    pub error: Option<String>,
    pub init_res: Option<InitializeResult>,
    pub tools: Vec<String>,
}
impl UpstreamMcpSummary {
    async fn new(server: &ServerConfig) -> Self {
        let (error, init_res, tools) = match server.connect().await {
            Ok(client) => {
                let mut error = None;
                let init_result = client.peer_info().cloned();
                let tool_names = match client.list_all_tools().await {
                    Ok(tools) => tools.into_iter().map(|t| t.name.to_string()).collect(),
                    Err(e) => {
                        error = Some(format!("Failed listing tools: {e}"));
                        vec![]
                    }
                };
                let _ = client.cancel().await;

                (error, init_result, tool_names)
            }
            Err(McpConnectionError::RequiresAuth) => {
                (Some("Requires authentication".into()), None, vec![])
            }
            Err(McpConnectionError::Failed(msg)) => (Some(msg), None, vec![]),
        };

        Self {
            url: server.url.clone(),
            name: server.name.clone(),
            error,
            init_res,
            tools,
        }
    }
}
impl Display for UpstreamMcpSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fields = vec![];
        let url_field = format!("{}: {}", fmt_bold("URL"), &self.url);

        if let Some(e) = &self.error {
            fields.extend([fmt_error(e), url_field]);
        } else {
            fields.extend([fmt_success("Connected"), url_field]);

            if let Some(init_res) = &self.init_res {
                fields.push(format!(
                    "{}: {}",
                    fmt_bold("Upstream Name"),
                    &init_res.server_info.name
                ));
                fields.push(format!(
                    "{}: {}",
                    fmt_bold("Upstream Version"),
                    &init_res.server_info.version
                ));
                fields.push(format!(
                    "{}: {}",
                    fmt_bold("Upstream Title"),
                    init_res
                        .server_info
                        .title
                        .clone()
                        .unwrap_or(fmt_dimmed("none"))
                ));

                let instructions = init_res
                    .instructions
                    .as_ref()
                    .map_or(fmt_dimmed("none"), |i| {
                        format!("{}...", i.chars().take(100).collect::<String>())
                    });
                fields.push(format!(
                    "{}: {instructions}",
                    fmt_bold("Upstream Instructions"),
                ));
            }

            if self.tools.is_empty() {
                fields.push(format!("{}: {}", fmt_bold("Tools"), fmt_dimmed("none")));
            } else {
                let tool_display = self
                    .tools
                    .iter()
                    .take(5)
                    .map(|t| fmt_green(t))
                    .collect::<Vec<String>>()
                    .join(", ");

                fields.push(format!(
                    "{} ({}): {tool_display}{}",
                    fmt_bold("Tools"),
                    self.tools.len(),
                    if self.tools.len() > 5 {
                        format!(", {}", fmt_green("..."))
                    } else {
                        String::new()
                    }
                ));
            }
        }

        let tree = fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                if i < fields.len() - 1 {
                    format!("├── {f}")
                } else {
                    format!("└── {f}")
                }
            })
            .collect::<Vec<String>>()
            .join("\n");

        write!(f, "{}\n{tree}", fmt_cyan(&self.name))
    }
}
