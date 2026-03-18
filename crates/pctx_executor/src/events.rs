use std::time::SystemTime;

pub use pctx_registry::events::*;
use serde::Deserialize;

use crate::{Diagnostic, Serialize};

/// The outcome of the type-check phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypecheckOutcome {
    #[serde(rename = "passed")]
    Passed,
    #[serde(rename = "failed")]
    Failed { diagnostics: Vec<Diagnostic> },
}

/// A timed record of the type-check phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCheckEvent {
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub outcome: TypecheckOutcome,
}

/// A single event in an execution flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecutionEvent {
    #[serde(rename = "type_check")]
    TypeCheck(TypeCheckEvent),
    #[serde(rename = "mcp_tool_call")]
    McpToolCall(McpToolCallEvent),
    #[serde(rename = "callback_invocation")]
    CallbackInvocation(CallbackInvocationEvent),
}

impl ExecutionEvent {
    pub fn started_at(&self) -> SystemTime {
        match self {
            ExecutionEvent::TypeCheck(e) => e.started_at,
            ExecutionEvent::McpToolCall(e) => e.started_at,
            ExecutionEvent::CallbackInvocation(e) => e.started_at,
        }
    }
}

/// A complete trace of one TypeScript execution flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// The original TypeScript script that was submitted.
    pub code: String,
    /// When `execute()` was called.
    pub started_at: SystemTime,
    /// When `execute()` returned.
    pub ended_at: SystemTime,
    /// All events that occurred during the flow, sorted by `started_at`.
    pub events: Vec<ExecutionEvent>,
}

impl Default for ExecutionTrace {
    fn default() -> Self {
        Self {
            code: String::new(),
            started_at: SystemTime::UNIX_EPOCH,
            ended_at: SystemTime::UNIX_EPOCH,
            events: Vec::new(),
        }
    }
}
