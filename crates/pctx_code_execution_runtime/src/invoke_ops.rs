//! Deno ops for local tool callback functionality
//!
//! Callbacks are pre-registered callbacks that execute synchronously.
//! Callbacks handle their own execution logic (WebSocket RPC, MCP calls, etc.)

use deno_core::{OpState, op2};
use rmcp::model::JsonObject;
use std::cell::RefCell;
use std::rc::Rc;

use pctx_registry::{PctxRegistry, RegistryError};

#[op2(async)]
#[serde]
pub(crate) async fn op_invoke(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
    #[serde] arguments: Option<JsonObject>,
) -> Result<serde_json::Value, RegistryError> {
    let registry = {
        let borrowed = state.borrow();
        borrowed.borrow::<PctxRegistry>().clone()
    };

    registry.invoke(&id, arguments).await
}
