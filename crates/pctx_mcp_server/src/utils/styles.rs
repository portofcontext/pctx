use anstyle::{Color, Style};

pub(crate) fn fmt_cyan(msg: &str) -> String {
    let style = Style::new().fg_color(Some(Color::Ansi(anstyle::AnsiColor::BrightCyan)));
    format!("{style}{msg}{style:#}")
}

pub(crate) fn fmt_dimmed(msg: &str) -> String {
    let style = Style::new().dimmed();
    format!("{style}{msg}{style:#}")
}
