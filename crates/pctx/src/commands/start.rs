use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use pctx_session_server::{AppState, start_server};
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
use tracing::info;

use crate::utils::styles::fmt_dimmed;

const LOGO: &str = include_str!("../../../../assets/ascii-logo.txt");

#[derive(Debug, Clone, Parser)]
pub struct StartCmd {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host address to bind to (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Path to session storage directory
    #[arg(long, default_value = ".pctx/sessions")]
    pub session_dir: Utf8PathBuf,

    /// Don't show the server banner
    #[arg(long)]
    pub no_banner: bool,
}

impl StartCmd {
    pub(crate) async fn handle(&self) -> Result<()> {
        let state = AppState::new_local();

        self.print_banner();

        start_server(&self.host, self.port, state).await?;

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
    }
}
