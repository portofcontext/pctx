use std::{collections::HashMap, sync::Arc, time::Duration};

use rmcp::model::RequestId;
use tokio::sync::{RwLock, mpsc as tokio_mpsc};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::model::{ExecuteToolParams, ExecuteToolResult, PctxJsonRpcRequest, WsJsonRpcMessage};

#[derive(Debug, thiserror::Error)]
pub enum ExecuteCallbackError {
    #[error("Failed to send execution request")]
    SendFailed,
    #[error("Execution failed: {0}")]
    ExecutionFailed(rmcp::model::ErrorData),
    #[error("Response channel closed")]
    ChannelClosed,
    /// `tool` is the registry id (`namespace__name`), matching the key callers
    /// use in `tool_timeout_overrides`.
    #[error("Tool `{tool}` timed out after {}s", .timeout.as_secs())]
    Timeout { tool: String, timeout: Duration },
}

#[derive(Default)]
pub struct WsManager {
    /// Active sessions by ID
    pub(crate) sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<WsSession>>>>>,
}

impl WsManager {
    /// Lists current sessions
    pub async fn list_sessions(&self) -> Vec<Uuid> {
        self.sessions.read().await.keys().copied().collect()
    }

    /// Add a new session
    pub async fn add(&self, session: WsSession) -> Uuid {
        let session_id = session.id;
        let session_lock = Arc::new(RwLock::new(session));
        self.sessions.write().await.insert(session_id, session_lock);
        session_id
    }

    /// Remove a session and cancel all pending executions
    pub async fn remove_session(&self, session_id: Uuid) {
        // First, get the session and clear its pending executions
        // This will drop all response_tx senders, causing waiting recv() calls
        // to immediately fail with ChannelClosed instead of waiting for timeout
        {
            let sessions = self.sessions.read().await;
            if let Some(session_lock) = sessions.get(&session_id) {
                let session = session_lock.read().await;
                session.pending_executions.write().await.clear();
                debug!("Cleared pending executions for session {session_id}");
            }
        }

        // Then remove the session from the map
        let mut sessions = self.sessions.write().await;
        sessions.remove(&session_id);
    }

    pub async fn get_for_code_mode_session(
        &self,
        code_mode_session_id: Uuid,
    ) -> Option<Arc<RwLock<WsSession>>> {
        let sessions = self.sessions.read().await;

        // Search through sessions to find one with matching code_mode_session_id
        for session_lock in sessions.values() {
            let session = session_lock.read().await;
            if session.code_mode_session_id == code_mode_session_id {
                // Clone the Arc before dropping locks
                return Some(session_lock.clone());
            }
        }

        None
    }

    /// Handle a response from a client for a pending execution
    /// Finds the session with the matching `request_id` and delegates to it
    pub async fn handle_execute_callback_response(
        &self,
        request_id: RequestId,
        result: Result<ExecuteToolResult, rmcp::model::ErrorData>,
    ) -> Result<(), ()> {
        let sessions = self.sessions.read().await;

        // Find the session that has this pending execution
        for session_lock in sessions.values() {
            let session_read = session_lock.read().await;

            if session_read
                .pending_executions
                .read()
                .await
                .contains_key(&request_id)
            {
                // Handle the response on the cloned Arc
                session_read
                    .handle_execute_callback_response(request_id, result.clone())
                    .await?;
                return Ok(());
            }
        }

        warn!("No session found with pending execution for request_id: {request_id}");
        Err(())
    }
}

type PendingExecutionsMap = Arc<
    RwLock<
        HashMap<
            RequestId,
            std::sync::mpsc::Sender<Result<ExecuteToolResult, rmcp::model::ErrorData>>,
        >,
    >,
>;

/// WebSocket session representing a connected client
#[derive(Clone)]
pub struct WsSession {
    pub id: Uuid,
    pub code_mode_session_id: Uuid,
    /// Channel to send messages to the client
    pub sender: tokio_mpsc::UnboundedSender<WsJsonRpcMessage>,
    /// Pending execution requests waiting for responses
    pending_executions: PendingExecutionsMap,
}
impl WsSession {
    pub fn new(
        sender: tokio_mpsc::UnboundedSender<WsJsonRpcMessage>,
        code_mode_session_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            code_mode_session_id,
            pending_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a callback on this session, sending a message and waiting for a response
    pub async fn execute_callback(
        &self,
        params: ExecuteToolParams,
        timeout: Duration,
    ) -> Result<ExecuteToolResult, ExecuteCallbackError> {
        let req_id = RequestId::String(Uuid::new_v4().to_string().into());
        let tool_id = params.tool_id();
        // Create std::sync::mpsc channel for response
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        // Store pending execution
        self.pending_executions
            .write()
            .await
            .insert(req_id.clone(), response_tx);

        debug!(
            request_id = ?req_id,
            tool = %tool_id,
            ws_session_id = %self.id,
            "Dispatching callback request to client",
        );

        // Send message to client
        self.sender
            .send(WsJsonRpcMessage::request(
                PctxJsonRpcRequest::ExecuteTool { params },
                req_id.clone(),
            ))
            .map_err(|_| ExecuteCallbackError::SendFailed)?;

        // Wait for response with timeout
        let result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || response_rx.recv()),
        )
        .await;

        // Clean up pending execution
        self.pending_executions.write().await.remove(&req_id);

        match result {
            Ok(Ok(Ok(Ok(value)))) => Ok(value),
            Ok(Ok(Ok(Err(error)))) => Err(ExecuteCallbackError::ExecutionFailed(error)),
            Ok(Ok(Err(_))) => Err(ExecuteCallbackError::ChannelClosed),
            Ok(Err(_)) => Err(ExecuteCallbackError::ChannelClosed),
            Err(_) => Err(ExecuteCallbackError::Timeout {
                tool: tool_id,
                timeout,
            }),
        }
    }

    /// Handle a response from a client for a pending execution
    pub async fn handle_execute_callback_response(
        &self,
        request_id: RequestId,
        result: Result<ExecuteToolResult, rmcp::model::ErrorData>,
    ) -> Result<(), ()> {
        let pending_read = self.pending_executions.read().await;
        debug!(
            request_id = ?request_id,
            ws_session_id = %self.id,
            pending_count = pending_read.len(),
            "Handling callback response from client",
        );
        if let Some(response_tx) = pending_read.get(&request_id) {
            debug!("Found pending execution, sending result");
            let send_result = response_tx.send(result);
            debug!("mpsc send result: {send_result:?}");
            Ok(())
        } else {
            warn!("No pending execution found for request_id: {request_id:?}");
            Err(())
        }
    }
}
