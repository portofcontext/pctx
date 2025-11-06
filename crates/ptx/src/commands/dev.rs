use clap::Parser;

use crate::mcp::{PtxMcp, tools::UpstreamMcp};

#[derive(Parser, Clone, Debug)]
/// Run PTX development MPC server
pub(crate) struct DevCmd {
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

impl DevCmd {
    pub(crate) async fn handle(&self) {
        let ctx = include_str!("./ctx.json");
        let upstream: UpstreamMcp = serde_json::from_str(ctx).expect("invalid format");

        PtxMcp::serve(&self.host, self.port, upstream).await;
    }
}
