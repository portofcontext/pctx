mod code_mode;
pub mod model;
pub mod parallel_registration;
pub mod search;

pub use code_mode::CodeMode;
use pctx_codegen::CodegenError;
pub use search::{SearchResult, ToolSearchIndex};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MCP Connection error: {0}")]
    McpConnection(#[from] pctx_config::server::McpConnectionError),
    #[error("MCP Service error: {0}")]
    McpService(#[from] pctx_config::server::ServiceError),
    #[error("Codegen error: {0}")]
    Codegen(#[from] CodegenError),
    #[error("Execution error: {0:?}")]
    Execution(#[from] pctx_executor::DenoExecutorError),
    #[error("Error: {0}")]
    Message(String),
}
