use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

/// The outcome of a registry action invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventOutcome {
    #[serde(rename = "success")]
    Success { output: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallEvent {
    pub server: String,
    pub tool: String,
    /// Whether the MCP client was served from the connection pool cache.
    pub cached_client: bool,
    pub args: Option<serde_json::Value>,
    pub outcome: EventOutcome,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallbackInvocationEvent {
    pub id: String,
    pub args: Option<serde_json::Value>,
    pub outcome: EventOutcome,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
}

/// A single recorded registry interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RegistryEvent {
    #[serde(rename = "mcp_tool_call")]
    McpToolCall(McpToolCallEvent),
    #[serde(rename = "callback_invocation")]
    CallbackInvocation(CallbackInvocationEvent),
}

impl RegistryEvent {
    pub fn started_at(&self) -> SystemTime {
        match self {
            RegistryEvent::McpToolCall(e) => e.started_at,
            RegistryEvent::CallbackInvocation(e) => e.started_at,
        }
    }
}

/// An append-only, cheaply-cloneable log of registry interactions.
///
/// All clones share the same underlying entry list. Use [`events`] to
/// snapshot the current contents.
///
/// [`events`]: RegistryTrace::events
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct RegistryTrace {
    events: Arc<Mutex<Vec<RegistryEvent>>>,
}

impl RegistryTrace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, event: RegistryEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }

    /// Returns a snapshot of all recorded events.
    pub fn events(&self) -> Vec<RegistryEvent> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.events.lock().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
