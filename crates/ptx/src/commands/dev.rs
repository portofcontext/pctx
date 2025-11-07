use anyhow::Result;
use clap::Parser;

use crate::mcp::{
    PtxMcp,
    inspect::inspect_mcp_server,
    tools::{UpstreamMcp, UpstreamTool},
};

#[derive(Parser, Clone, Debug)]
/// Run PTX development MPC server
pub(crate) struct DevCmd {
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
    #[arg(long)]
    url: Vec<String>,
}

impl DevCmd {
    pub(crate) async fn handle(&self) -> Result<()> {
        let ctx = include_str!("./ctx.json");

        let mut upstream: Vec<UpstreamMcp> = vec![];
        for url in &self.url {
            let (info, tools) = inspect_mcp_server(url).await;

            println!(
                "Generating typescript interface for {name}({url}) containing {tools_len} tools",
                name = &info.name,
                tools_len = tools.len(),
            );
            for tool in tools {
                println!("{}", &tool.name);
                UpstreamTool::from_tool(tool);
            }
        }

        PtxMcp::serve(&self.host, self.port, upstream).await;

        Ok(())
    }
}
