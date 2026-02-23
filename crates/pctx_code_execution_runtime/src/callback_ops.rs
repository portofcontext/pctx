//! Deno ops for local tool callback functionality
//!
//! Callbacks are pre-registered callbacks that execute synchronously.
//! Callbacks handle their own execution logic (WebSocket RPC, MCP calls, etc.)

use deno_core::{OpState, op2};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{CallbackRegistry, error::McpError};

#[op2(async)]
#[serde]
pub(crate) async fn op_invoke_callback(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
    #[serde] arguments: Option<serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let registry = {
        let borrowed = state.borrow();
        borrowed.borrow::<CallbackRegistry>().clone()
    };

    registry
        .invoke(&id, arguments)
        .await
        .map_err(McpError::ExecutionError)
}
