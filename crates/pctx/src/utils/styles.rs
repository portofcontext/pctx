#![allow(dead_code)]
use crate::utils::{CHECK, MARK};

#[allow(dead_code)]
pub(crate) mod color {
    use anstyle::*;

    pub(crate) const NOP: Style = Style::new();
    pub(crate) const GREEN_BOLD: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
    pub(crate) const CYAN_BOLD: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
    pub(crate) const CYAN: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
    pub(crate) const BLUE_BOLD: Style = AnsiColor::BrightBlue.on_default().effects(Effects::BOLD);
    pub(crate) const RED_BOLD: Style = AnsiColor::BrightRed.on_default().effects(Effects::BOLD);
    pub(crate) const YELLOW: Style = AnsiColor::Yellow.on_default();
    pub(crate) const BOLD: Style = anstyle::Style::new().bold();
    pub(crate) const DIMMED: Style = anstyle::Style::new().dimmed();
}

pub(crate) fn get_styles() -> clap::builder::Styles {
    clap::builder::styling::Styles::styled()
        .header(color::GREEN_BOLD)
        .usage(color::GREEN_BOLD)
        .literal(color::CYAN_BOLD)
        .placeholder(color::CYAN)
        .error(color::RED_BOLD)
        .valid(color::CYAN_BOLD)
        .invalid(color::YELLOW)
}

pub(crate) fn fmt_style(msg: &str, style: &anstyle::Style) -> String {
    format!("{style}{msg}{style:#}")
}

macro_rules! make_fmt {
    ($($fn_name:ident => $const:ident),* $(,)?) => {
        $(pub(crate) fn $fn_name(msg: &str) -> String {
            fmt_style(msg, &color::$const)
        })*
    };
}

make_fmt! {
    fmt_nop             => NOP,
    fmt_green_bold      => GREEN_BOLD,
    fmt_cyan_bold       => CYAN_BOLD,
    fmt_cyan            => CYAN,
    fmt_blue_bold       => BLUE_BOLD,
    fmt_red_bold        => RED_BOLD,
    fmt_yellow          => YELLOW,
    fmt_bold            => BOLD,
    fmt_dimmed          => DIMMED,
}

pub(crate) fn fmt_good_check(msg: &str) -> String {
    format!("{} {msg}", fmt_green_bold(CHECK))
}

pub(crate) fn fmt_error_x(msg: &str) -> String {
    format!("{} {msg}", fmt_red_bold(MARK))
}
