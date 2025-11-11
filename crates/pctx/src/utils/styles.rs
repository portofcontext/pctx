use anstyle::{AnsiColor, Color, Style};
use clap::builder::Styles;

use crate::utils::{CHECK, MARK};

pub fn get_styles() -> Styles {
    Styles::styled()
        .usage(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Blue))),
        )
        .header(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Blue))),
        )
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
        .invalid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .valid(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::White))))
}

fn fmt_style(msg: &str, style: &Style) -> String {
    format!("{style}{msg}{style:#}")
}

pub(crate) fn fmt_green(msg: &str) -> String {
    let green = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    fmt_style(msg, &green)
}

pub(crate) fn fmt_cyan(msg: &str) -> String {
    let cyan = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightCyan)));
    fmt_style(msg, &cyan)
}

pub(crate) fn fmt_red(msg: &str) -> String {
    let red = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));
    fmt_style(msg, &red)
}

pub(crate) fn fmt_yellow(msg: &str) -> String {
    let yellow = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
    fmt_style(msg, &yellow)
}

pub(crate) fn fmt_bold(msg: &str) -> String {
    let bold = Style::new().bold();
    fmt_style(msg, &bold)
}

pub(crate) fn fmt_dimmed(msg: &str) -> String {
    let bold = Style::new().dimmed();
    fmt_style(msg, &bold)
}

pub(crate) fn fmt_success(msg: &str) -> String {
    format!("{} {msg}", fmt_green(CHECK))
}

pub(crate) fn fmt_error(msg: &str) -> String {
    format!("{} {msg}", fmt_red(MARK))
}
