mod callback_registry;
mod error;
mod mcp_registry;

pub use callback_registry::{CallbackFn, CallbackRegistry};
pub use error::RegistryError;
pub use mcp_registry::{MCPRegistry, call_mcp_tool};
