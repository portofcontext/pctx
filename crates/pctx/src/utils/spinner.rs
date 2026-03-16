use crate::utils::{CHECK, MARK};

use super::styles::{fmt_green_bold, fmt_red_bold, fmt_yellow};
use spinoff::{Color, spinners};
use std::borrow::Cow;
use tracing::log::{Level, error, info, log_enabled, warn};

/// Wrapper around `spinoff::Spinner` to handle only
/// showing if log level is INFO
pub(crate) struct Spinner {
    sp: Option<spinoff::Spinner>,
}

#[allow(unused)]
impl Spinner {
    pub(crate) fn new<M: Into<Cow<'static, str>>>(msg: M) -> Self {
        let sp = if log_enabled!(Level::Debug) || !log_enabled!(Level::Info) {
            // level debug or quiet mode
            info!("⣯ {}", msg.into());
            None
        } else {
            Some(spinoff::Spinner::new(spinners::Dots, msg, Color::Blue))
        };
        Self { sp }
    }

    pub(crate) fn update_text<M: Into<Cow<'static, str>>>(&mut self, msg: M) {
        if let Some(sp) = self.sp.as_mut() {
            sp.update_text(msg);
        } else {
            info!("{}...", msg.into());
        }
    }

    pub(crate) fn stop_and_persist<M: Into<Cow<'static, str>>>(&mut self, symbol: &str, msg: M) {
        if let Some(sp) = self.sp.as_mut() {
            sp.stop_and_persist(symbol, &msg.into());
        } else {
            info!("{symbol} {}", msg.into());
        }
    }

    pub(crate) fn stop_success<M: Into<Cow<'static, str>>>(&mut self, msg: M) {
        let symbol = fmt_green_bold(CHECK);
        if let Some(sp) = self.sp.as_mut() {
            sp.stop_and_persist(&symbol, &msg.into());
        } else {
            info!("{symbol} {}", msg.into());
        }
    }

    pub(crate) fn stop_warn<M: Into<Cow<'static, str>>>(&mut self, msg: M) {
        let symbol = fmt_yellow("ø");
        if let Some(sp) = self.sp.as_mut() {
            sp.stop_and_persist(&symbol, &msg.into());
        } else {
            warn!("{symbol} {}", msg.into());
        }
    }

    pub(crate) fn stop_error<M: Into<Cow<'static, str>>>(&mut self, msg: M) {
        let symbol = fmt_red_bold(MARK);
        if let Some(sp) = self.sp.as_mut() {
            sp.stop_and_persist(&symbol, &msg.into());
        } else {
            error!("{symbol} {}", msg.into());
        }
    }
}
