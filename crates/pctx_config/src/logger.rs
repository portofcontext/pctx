use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig {
    #[serde(default = "crate::defaults::default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default)]
    pub format: LoggerFormat,
    #[serde(default = "crate::defaults::default_true")]
    pub colors: bool,
    /// Optional file path for log output. If not specified:
    /// - stdio mode: logging is disabled (keeps stdout/stderr clean for JSON-RPC)
    /// - other modes: logs to stdout
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum LoggerFormat {
    #[serde(rename = "compact")]
    #[default]
    Compact,
    #[serde(rename = "pretty")]
    Pretty,
    #[serde(rename = "json")]
    Json,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Info,
            format: LoggerFormat::Compact,
            colors: true,
            file: None,
        }
    }
}

/// Define an enumeration for log levels
/// Ordered from lowest to highest severity: Trace < Debug < Info < Warn < Error
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// The "trace" level.
    #[serde(rename = "trace", alias = "TRACE")]
    Trace,
    /// The "debug" level.
    #[serde(rename = "debug", alias = "DEBUG")]
    Debug,
    /// The "info" level.
    #[serde(rename = "info", alias = "INFO")]
    #[default]
    Info,
    /// The "warn" level.
    #[serde(rename = "warn", alias = "WARN")]
    Warn,
    /// The "error" level.
    #[serde(rename = "error", alias = "ERROR")]
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}
