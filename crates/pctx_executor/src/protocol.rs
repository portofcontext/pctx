/// IPC message types exchanged between the pool manager (parent process) and
/// worker processes over newline-delimited JSON on stdin/stdout.
///
/// Parent → Worker: [`WorkerRequest`]
/// Worker → Parent: [`WorkerMessage`]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Diagnostic, ExecutionError, events::ExecutionEvent};

// ---------------------------------------------------------------------------
// Parent → Worker
// ---------------------------------------------------------------------------

/// A message sent from the pool manager to a worker.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerRequest {
    /// Begin executing the given code.
    Execute(ExecuteRequest),
    /// Return value for a callback the worker asked the parent to invoke.
    CallbackResponse(CallbackResponse),
}

/// Payload for a [`WorkerRequest::Execute`] message.
///
/// Instead of sending MCP server configs and having the worker reconnect,
/// all tool IDs (both callbacks and MCP tools) are sent as `all_tool_ids`.
/// The worker creates an IPC-proxy stub for each ID so every tool call is
/// routed back to the parent's registry, which holds the live connection pool.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// Correlates this request with the eventual [`ExecuteResultMsg`].
    pub request_id: Uuid,
    /// Fully-prepared TypeScript code (post code-generation wrapping).
    pub code: String,
    /// Every tool ID registered in the parent's registry — both MCP tool IDs
    /// (formatted as `"server__tool"`) and callback IDs.  The worker creates
    /// an IPC-proxy callback for each so no direct connections leave the worker.
    pub all_tool_ids: Vec<String>,
}

/// The parent's answer to a [`CallbackRequest`] the worker sent earlier.
#[derive(Debug, Serialize, Deserialize)]
pub struct CallbackResponse {
    /// Matches the `callback_call_id` in the original [`CallbackRequest`].
    pub callback_call_id: Uuid,
    /// `Ok(value)` on success, `Err(message)` on failure.
    pub result: Result<serde_json::Value, String>,
}

// ---------------------------------------------------------------------------
// Worker → Parent
// ---------------------------------------------------------------------------

/// A message sent from a worker to the pool manager.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerMessage {
    /// V8 platform is initialised; the worker is ready to accept requests.
    Ready,
    /// Execution has finished (success or failure).
    ExecuteResult(ExecuteResultMsg),
    /// A callback registered in the parent must be invoked.
    CallbackRequest(CallbackRequest),
}

/// Payload for a [`WorkerMessage::ExecuteResult`] message.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResultMsg {
    /// Matches the `request_id` from the originating [`ExecuteRequest`].
    pub request_id: Uuid,
    pub success: bool,
    /// Type-check diagnostics (non-empty only when type checking failed).
    pub diagnostics: Vec<Diagnostic>,
    /// Runtime error (set when execution threw an unhandled exception).
    pub runtime_error: Option<ExecutionError>,
    /// Default export value from the module.
    pub output: Option<serde_json::Value>,
    pub stdout: String,
    pub stderr: String,
    /// Full ordered event log (TypeCheck + registry events), already sorted
    /// by `started_at`.
    pub events: Vec<ExecutionEvent>,
}

/// A request from the worker to invoke a parent-side callback.
#[derive(Debug, Serialize, Deserialize)]
pub struct CallbackRequest {
    /// Unique ID for this specific invocation (used to match the response).
    pub callback_call_id: Uuid,
    /// The callback ID as registered in the parent's [`PctxRegistry`].
    pub callback_id: String,
    /// Arguments passed from TypeScript.
    pub args: Option<serde_json::Value>,
}
