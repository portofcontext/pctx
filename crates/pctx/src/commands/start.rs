use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use pctx_code_mode::{ExecutorPool, PoolConfig};
use pctx_session_server::{AppState, start_server};
use std::sync::Arc;
use tabled::{
    Table,
    builder::Builder,
    settings::{
        Alignment, Color, Panel, Style, Width,
        object::{Cell, Columns, Rows},
        peaker::Priority,
        width::MinWidth,
    },
};
use terminal_size::terminal_size;
use tracing::{info, warn};
use url::Url;

use crate::utils::styles::fmt_dimmed;

const LOGO: &str = include_str!("../../../../assets/ascii-logo.txt");

#[derive(Debug, Clone, Parser)]
pub struct StartCmd {
    /// Port to listen on
    #[arg(short, long, default_value = "8080", env = "PCTX_PORT")]
    pub port: u16,

    /// Host address to bind to (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1", env = "PCTX_HOST")]
    pub host: String,

    /// Path to session storage directory
    #[arg(long, default_value = ".pctx/sessions")]
    pub session_dir: Utf8PathBuf,

    /// Allowed CORS origins. Can be specified multiple times.
    /// Defaults to localhost only (<http://localhost>, <http://127.0.0.1>, http://[`::1`]).
    /// Specify your own origins to override the default (can include or exclude localhost).
    /// Origins without explicit ports will match any port.
    /// Example: --allowed-origin <http://localhost> --allowed-origin <https://app.example.com>
    #[arg(long = "allowed-origin")]
    pub allowed_origins: Vec<Url>,

    /// Number of worker processes in the executor pool.
    /// Defaults to the number of logical CPUs, capped at 8.
    /// Set to 0 to disable the pool and run in-process.
    #[arg(long, env = "PCTX_WORKERS")]
    pub workers: Option<usize>,

    /// Don't show the server banner
    #[arg(long)]
    pub no_banner: bool,
}

impl StartCmd {
    pub(crate) async fn handle(&self) -> Result<()> {
        let worker_count = self.workers.unwrap_or_else(default_workers);
        let state = if worker_count == 0 {
            info!("Executor pool disabled (--workers 0), running in-process");
            AppState::new_local()
        } else {
            match PoolConfig::from_current_exe(worker_count) {
                Ok(pool_cfg) => match ExecutorPool::new(pool_cfg).await {
                    Ok(pool) => {
                        info!("Executor pool ready ({worker_count} workers)");
                        AppState::new_local().with_executor_pool(Arc::new(pool))
                    }
                    Err(e) => {
                        warn!("Failed to start executor pool, falling back to in-process execution: {e}");
                        AppState::new_local()
                    }
                },
                Err(e) => {
                    warn!("Could not locate worker binary, falling back to in-process execution: {e}");
                    AppState::new_local()
                }
            }
        };

        self.print_banner();

        // Use default localhost origins if none specified
        let allowed_origins: Vec<String> = if self.allowed_origins.is_empty() {
            vec![
                "http://localhost".to_string(),
                "http://127.0.0.1".to_string(),
                "http://[::1]".to_string(),
            ]
        } else {
            self.allowed_origins
                .iter()
                .map(std::string::ToString::to_string)
                .collect()
        };

        start_server(&self.host, self.port, state, allowed_origins).await?;

        Ok(())
    }

    fn print_banner(&self) {
        let rest_url = format!("http://{}:{}", self.host, self.port);
        let ws_url = format!("ws://{}:{}/ws", self.host, self.port);

        let logo_max_length = LOGO
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let min_term_width = logo_max_length + 4;
        let term_width = terminal_size().map(|(w, _)| w.0).unwrap_or_default() as usize;

        if !self.no_banner && term_width >= min_term_width {
            let mut builder = Builder::default();

            builder.push_record(["Mode", "Agent"]);
            builder.push_record(["REST API", &rest_url]);
            builder.push_record(["WebSocket", &ws_url]);
            builder.push_record(["Docs", &fmt_dimmed("https://github.com/portofcontext/pctx")]);

            let table_width = (term_width).min(80) as usize;
            let info_table = builder
                .build()
                .with(Style::empty())
                .modify(Columns::first(), Color::BOLD)
                .modify(Cell::new(1, 1), Color::FG_CYAN) // REST API URL
                .modify(Columns::first(), MinWidth::new(20))
                .modify(Columns::new(..2), Width::wrap((term_width - 6) / 2))
                .to_string();

            let logo_panel = Panel::header(format!("\n{LOGO}\n\n"));
            let logo_row = 0;
            let version_panel = Panel::header(format!(
                "pctx v{}\n\n",
                option_env!("CARGO_PKG_VERSION").unwrap_or_default()
            ));
            let version_row = 1;

            let style = Style::rounded().remove_horizontals().remove_vertical();
            let banner = Table::from_iter([[info_table]])
                .with(style)
                .with(version_panel)
                .with(logo_panel)
                .with(Alignment::center())
                .modify(Rows::new(logo_row..=logo_row), Color::FG_BLUE)
                .modify(
                    Rows::new(version_row..=version_row),
                    Color::FG_BLUE | Color::BOLD,
                )
                .with((
                    Width::wrap(table_width).priority(Priority::max(true)),
                    Width::increase(table_width).priority(Priority::min(true)),
                ))
                .to_string();

            println!("\n{banner}\n");
        }

        info!("pctx agent server listening at {rest_url}...");
    }
}

/// Default worker count: logical CPUs, capped at 8.
fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4)
}
