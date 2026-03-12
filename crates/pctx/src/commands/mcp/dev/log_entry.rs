use std::collections::HashMap;

use chrono::{DateTime, Utc};
use pctx_config::logger::LogLevel;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LogEntry {
    pub(super) timestamp: DateTime<Utc>,
    pub(super) level: LogLevel,
    #[allow(unused)]
    pub(super) target: String,
    pub(super) fields: LogEntryFields,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LogEntryFields {
    #[serde(default)]
    pub(super) message: String,
    #[serde(flatten)]
    pub(super) extra: HashMap<String, serde_json::Value>,
}

impl LogEntry {
    pub(super) fn prefix(&self) -> String {
        self.level.as_str().to_uppercase()
    }

    pub(super) fn color(&self) -> Color {
        match &self.level {
            LogLevel::Trace => Color::LightMagenta,
            LogLevel::Debug => Color::Blue,
            LogLevel::Info => Color::Green,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::Red,
        }
    }

    pub(super) fn tui_line(&'_ self, level: LogLevel) -> Line<'_> {
        let time_str = self.timestamp.format("%H:%M:%S").to_string();
        let mut parts = vec![Span::styled(
            format!("[{time_str}] "),
            Style::default().dark_gray(),
        )];
        if level <= LogLevel::Debug {
            parts.push(Span::styled(
                format!("{} ", &self.target),
                Style::default().dark_gray(),
            ));
        }
        parts.extend([
            Span::styled(
                format!("[{}] ", self.prefix()),
                Style::default().fg(self.color()).bold(),
            ),
            Span::raw(self.fields.message.clone()),
        ]);

        Line::from(parts)
    }
}
