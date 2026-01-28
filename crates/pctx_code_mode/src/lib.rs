//! # PCTX Code Mode
//!
//! A TypeScript code execution engine that enables AI agents to dynamically call tools through generated code.
//! Code Mode converts tool schemas (like MCP tools) into TypeScript interfaces, executes LLM-generated code
//! in a sandboxed Deno runtime, and bridges function calls back to your Rust callbacks.
//!
//! ## Quick Start
//!
//! ```ignore
//! use pctx_code_mode::{CodeMode, CallbackRegistry};
//! use pctx_code_mode::model::CallbackConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 1. Define callback tools with JSON schemas
//!     let callback = CallbackConfig {
//!         namespace: "Greeter".to_string(),
//!         name: "greet".to_string(),
//!         description: Some("Greets a person by name".to_string()),
//!         input_schema: Some(json!({
//!             "type": "object",
//!             "properties": { "name": { "type": "string" } },
//!             "required": ["name"]
//!         })),
//!         output_schema: None,
//!     };
//!
//!     // 2. Create CodeMode instance and add callback
//!     let mut code_mode = CodeMode::default();
//!     code_mode.add_callback(&callback)?;
//!
//!     // 3. Register callback functions that execute when tools are called
//!     let registry = CallbackRegistry::default();
//!     registry.add("Greeter.greet", Arc::new(|args| {
//!         Box::pin(async move {
//!             let name = args
//!                 .and_then(|v| v.get("name"))
//!                 .and_then(|v| v.as_str())
//!                 .unwrap_or("World");
//!             Ok(serde_json::json!({ "message": format!("Hello, {name}!") }))
//!         })
//!     }))?;
//!
//!     // 4. Execute LLM-generated TypeScript code
//!     let code = r#"
//!         async function run() {
//!             const result = await Greeter.greet({ name: "Alice" });
//!             return result;
//!         }
//!     "#;
//!
//!     let output = code_mode.execute(code, Some(registry)).await?;
//!
//!     if output.success {
//!         println!("Result: {}", serde_json::to_string_pretty(&output.output)?);
//!     } else {
//!         eprintln!("Error: {}", output.stderr);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Core Concepts
//!
//! ### CodeMode
//!
//! The [`CodeMode`] struct is the main execution engine. It provides:
//!
//! **Builder methods** (chainable):
//! - [`CodeMode::with_server`] / [`CodeMode::with_servers`] - Add MCP servers
//! - [`CodeMode::with_callback`] / [`CodeMode::with_callbacks`] - Add callback tools
//!
//! **Registration methods** (mutable):
//! - [`CodeMode::add_server`] / [`CodeMode::add_servers`] - Add MCP servers
//! - [`CodeMode::add_callback`] / [`CodeMode::add_callbacks`] - Add callback tools
//! - [`CodeMode::add_tool_set`] - Add a pre-built ToolSet directly
//!
//! **Accessor methods**:
//! - [`CodeMode::tool_sets`] - Get registered ToolSets
//! - [`CodeMode::servers`] - Get registered server configurations
//! - [`CodeMode::callbacks`] - Get registered callback configurations
//! - [`CodeMode::allowed_hosts`] - Get allowed network hosts
//!
//! **Execution methods**:
//! - [`CodeMode::list_functions`] - List all available functions with minimal interfaces
//! - [`CodeMode::get_function_details`] - Get full typed interfaces for specific functions
//! - [`CodeMode::execute`] - Execute TypeScript code in the sandbox
//!
//! ### Tools and ToolSets
//!
//! [`Tool`]s represent individual functions callable from TypeScript.
//! They are organized into [`ToolSet`]s (namespaces). Tools can be:
//! - **MCP tools**: Loaded from MCP servers via [`CodeMode::add_server`]
//! - **Callback tools**: Defined via [`CallbackConfig`](model::CallbackConfig) and [`CodeMode::add_callback`]
//!
//! ### Callbacks
//!
//! [`CallbackFn`] are Rust async functions that execute when TypeScript code calls callback tools.
//! Register them in a [`CallbackRegistry`] and pass it to [`CodeMode::execute`].
//!
//! ## Examples
//!
//! ### Multi-Tool Workflow
//!
//! ```ignore
//! # use pctx_code_mode::{CodeMode, CallbackRegistry};
//! # async fn example(code_mode: CodeMode, registry: CallbackRegistry) -> anyhow::Result<()> {
//! let code = r#"
//!     async function run() {
//!         // Fetch user data
//!         const user = await UserApi.getUser({ id: 123 });
//!
//!         // Process the data
//!         const processed = await DataProcessor.transform({
//!             input: user.data,
//!             format: "normalized"
//!         });
//!
//!         // Save results
//!         const saved = await Storage.save({
//!             key: `user_${user.id}`,
//!             value: processed
//!         });
//!
//!         return {
//!             userId: user.id,
//!             saved: saved.success,
//!             location: saved.url
//!         };
//!     }
//! "#;
//!
//! let output = code_mode.execute(code, Some(registry)).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! 1. **Tool Definition**: Tools are defined with JSON Schemas for inputs/outputs
//! 2. **Code Generation**: TypeScript interface definitions are generated from schemas
//! 3. **Code Execution**: User code is wrapped with namespace implementations and executed in Deno
//! 4. **Callback Routing**: Function calls in TypeScript are routed to Rust callbacks or MCP servers
//! 5. **Result Marshaling**: JSON values are passed between TypeScript and Rust
//!
//! ## Sandbox Security
//!
//! Code is executed in a Deno runtime with:
//! - Network access restricted to allowed hosts (from registered MCP servers)
//! - No file system access
//! - No subprocess spawning
//! - Isolated V8 context per execution

mod code_mode;
pub mod model;

// Core execution API
pub use code_mode::CodeMode;

// Re-export config, runtime and codegen crates
pub use pctx_code_execution_runtime as runtime;
pub use pctx_codegen as codegen;
pub use pctx_config as config;

// Re-export commonly used types for backwards compatibility
pub use pctx_code_execution_runtime::{CallbackFn, CallbackRegistry};
pub use pctx_codegen::{RootSchema, Tool, ToolSet, case};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MCP Connection error: {0}")]
    McpConnection(#[from] pctx_config::server::McpConnectionError),
    #[error("MCP Service error: {0}")]
    McpService(#[from] pctx_config::server::ServiceError),
    #[error("Codegen error: {0}")]
    Codegen(#[from] pctx_codegen::CodegenError),
    #[error("Execution error: {0:?}")]
    Execution(#[from] pctx_executor::DenoExecutorError),
    #[error("Error: {0}")]
    Message(String),
}
