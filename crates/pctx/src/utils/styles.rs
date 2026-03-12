#![allow(dead_code)]
use crate::utils::{CHECK, MARK};

// CLI Styling copied from cargo

#[allow(dead_code)]
pub(crate) mod cargo_styles {
    use anstyle::*;

    pub(crate) const NOP: Style = Style::new();
    pub(crate) const HEADER: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
    pub(crate) const USAGE: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
    pub(crate) const LITERAL: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
    pub(crate) const PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
    pub(crate) const ERROR: Style = AnsiColor::BrightRed.on_default().effects(Effects::BOLD);
    pub(crate) const WARN: Style = AnsiColor::Yellow.on_default();
    pub(crate) const NOTE: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
    pub(crate) const GOOD: Style = AnsiColor::BrightGreen.on_default().effects(Effects::BOLD);
    pub(crate) const VALID: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
    pub(crate) const INVALID: Style = AnsiColor::Yellow.on_default();
    pub(crate) const TRANSIENT: Style = AnsiColor::BrightCyan.on_default().effects(Effects::BOLD);
    pub(crate) const CONTEXT: Style = AnsiColor::BrightBlue.on_default().effects(Effects::BOLD);
    pub(crate) const UPDATE_ADDED: Style = NOTE;
    pub(crate) const UPDATE_REMOVED: Style = ERROR;
    pub(crate) const UPDATE_UPGRADED: Style = GOOD;
    pub(crate) const UPDATE_DOWNGRADED: Style = WARN;
    pub(crate) const UPDATE_UNCHANGED: Style = anstyle::Style::new().bold();
    pub(crate) const DEP_NORMAL: Style = anstyle::Style::new().effects(anstyle::Effects::DIMMED);
    pub(crate) const DEP_BUILD: Style = anstyle::AnsiColor::Blue
        .on_default()
        .effects(anstyle::Effects::BOLD);
    pub(crate) const DEP_DEV: Style = anstyle::AnsiColor::Cyan
        .on_default()
        .effects(anstyle::Effects::BOLD);
    pub(crate) const DEP_FEATURE: Style = anstyle::AnsiColor::Magenta
        .on_default()
        .effects(anstyle::Effects::DIMMED);
}

pub(crate) fn get_styles() -> clap::builder::Styles {
    clap::builder::styling::Styles::styled()
        .header(cargo_styles::HEADER)
        .usage(cargo_styles::USAGE)
        .literal(cargo_styles::LITERAL)
        .placeholder(cargo_styles::PLACEHOLDER)
        .error(cargo_styles::ERROR)
        .valid(cargo_styles::VALID)
        .invalid(cargo_styles::INVALID)
}

pub(crate) fn fmt_style(msg: &str, style: &anstyle::Style) -> String {
    format!("{style}{msg}{style:#}")
}

macro_rules! make_fmt {
    ($($fn_name:ident => $const:ident),* $(,)?) => {
        $(pub(crate) fn $fn_name(msg: &str) -> String {
            fmt_style(msg, &cargo_styles::$const)
        })*
    };
}

make_fmt! {
    fmt_nop               => NOP,
    fmt_header            => HEADER,
    fmt_usage             => USAGE,
    fmt_literal           => LITERAL,
    fmt_placeholder       => PLACEHOLDER,
    fmt_warn              => WARN,
    fmt_note              => NOTE,
    fmt_good              => GOOD,
    fmt_valid             => VALID,
    fmt_invalid           => INVALID,
    fmt_transient         => TRANSIENT,
    fmt_context           => CONTEXT,
    fmt_update_added      => UPDATE_ADDED,
    fmt_update_removed    => UPDATE_REMOVED,
    fmt_update_upgraded   => UPDATE_UPGRADED,
    fmt_update_downgraded => UPDATE_DOWNGRADED,
    fmt_update_unchanged  => UPDATE_UNCHANGED,
    fmt_dep_normal        => DEP_NORMAL,
    fmt_dep_build         => DEP_BUILD,
    fmt_dep_dev           => DEP_DEV,
    fmt_dep_feature       => DEP_FEATURE,
    // ERROR is omitted — fmt_error adds an icon and has different semantics
    fmt_error               => ERROR,
}

pub(crate) fn fmt_bold(msg: &str) -> String {
    fmt_update_unchanged(msg)
}
pub(crate) fn fmt_dimmed(msg: &str) -> String {
    fmt_dep_normal(msg)
}

pub(crate) fn fmt_good_check(msg: &str) -> String {
    format!("{} {msg}", fmt_good(CHECK))
}

pub(crate) fn fmt_error_x(msg: &str) -> String {
    format!("{} {msg}", fmt_error(MARK))
}
