//! # PCTX Code Mode
//!
//! A TypeScript code execution engine that enables AI agents to dynamically call tools through generated code.
//! Code Mode converts tool schemas (like MCP tools) into TypeScript interfaces, executes LLM-generated code
//! in a sandboxed Deno runtime, and bridges function calls back to your Rust callbacks.
//!
//! ## Quick Start
//!
//! ```
//! use pctx_code_mode::CodeMode;
//! use pctx_code_mode::registry::PctxRegistry;
//! use pctx_code_mode::config::ToolDisclosure;
//! use pctx_code_mode::model::CallbackConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 1. Define callback tools with JSON schemas
//!     let callback = CallbackConfig {
//!         namespace: Some("Greeter".to_string()),
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
//!     let registry = PctxRegistry::default();
//!     registry.add_callback(&callback.id(), Arc::new(|args: Option<serde_json::Value>| {
//!         Box::pin(async move {
//!             let name = args.as_ref()
//!                 .and_then(|v| v.get("name"))
//!                 .and_then(|v| v.as_str())
//!                 .unwrap_or("World");
//!             Ok(serde_json::json!({ "message": format!("Hello, {name}!") }))
//!         })
//!     }));
//!
//!     // 4. Execute LLM-generated TypeScript code
//!     let code = r#"
//!         async function run() {
//!             const result = await Greeter.greet({ name: "Alice" });
//!             return result;
//!         }
//!     "#;
//!
//!     let output = code_mode.execute_typescript(code, ToolDisclosure::default(), Some(registry)).await?;
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
//! - [`CodeMode::tool_sets`] - Get all registered ToolSets
//! - [`CodeMode::server_tool_sets`] - Get only MCP server ToolSets
//! - [`CodeMode::servers`] - Get registered server configurations
//! - [`CodeMode::callbacks`] - Get registered callback configurations
//! - [`CodeMode::virtual_fs`] - Get the virtual filesystem used by bash execution
//!
//! **Execution methods**:
//! - [`CodeMode::list_functions`] - List all available functions with minimal interfaces
//! - [`CodeMode::get_function_details`] - Get full typed interfaces for specific functions
//! - [`CodeMode::execute_typescript`] - Execute TypeScript code in the sandbox
//! - [`CodeMode::execute_bash`] - Execute a bash command in the virtual filesystem
//!
//! ### ToolDisclosure
//!
//! [`config::ToolDisclosure`] controls how tools are presented to the LLM and how generated
//! TypeScript code invokes them. Choose the mode that matches your agent's workflow:
//!
//! - **`Catalog`** (default) - Tools are discovered via `list_tools` → `get_tool_details`, then
//!   called through typed TypeScript namespaces (e.g. `await Greeter.greet({ name: "Alice" })`).
//! - **`Filesystem`** - Like `Catalog` but the agent works within a virtual filesystem via
//!   `execute_bash` before invoking TypeScript.
//! - **`Sidecar`** - Tools are passed as original MCP descriptions. The generated code uses an
//!   `InvokeMap` type and a type-safe `invoke()` function rather than namespace methods.
//!
//! ### Tools and ToolSets
//!
//! [`Tool`]s represent individual functions callable from TypeScript.
//! They are organized into [`ToolSet`]s (namespaces). Tools can be:
//! - **MCP tools**: Loaded from MCP servers via [`CodeMode::add_server`]
//! - **Callback tools**: Defined via [`CallbackConfig`](model::CallbackConfig) and [`CodeMode::add_callback`]
//!
//! ### PctxRegistry
//!
//! [`PctxRegistry`] is a thread-safe registry that routes TypeScript function calls to either
//! local Rust callbacks or upstream MCP servers. Pass it to [`CodeMode::execute_typescript`].
//!
//! - `add_callback(id, fn)` - Register a [`CallbackFn`] for a tool ID (e.g. `"Greeter.greet"`)
//! - `add_mcp(tool_names, cfg)` - Register MCP tools from a server
//! - `invoke(id, args)` - Dispatch a call by tool ID (used internally during execution)
//!
//! ## Examples
//!
//! ### Multi-Tool Workflow
//!
//! ```
//! use pctx_code_mode::CodeMode;
//! use pctx_code_mode::registry::PctxRegistry;
//! use pctx_code_mode::config::ToolDisclosure;
//! async fn example(code_mode: CodeMode, registry: PctxRegistry) -> anyhow::Result<()> {
//!   let code = r#"
//!       async function run() {
//!           // Fetch user data
//!           const user = await UserApi.getUser({ id: 123 });
//!  
//!           // Process the data
//!           const processed = await DataProcessor.transform({
//!               input: user.data,
//!               format: "normalized"
//!           });
//!  
//!           // Save results
//!           const saved = await Storage.save({
//!               key: `user_${user.id}`,
//!               value: processed
//!           });
//!  
//!           return {
//!               userId: user.id,
//!               saved: saved.success,
//!               location: saved.url
//!           };
//!       }
//!   "#;
//!  
//!   let output = code_mode.execute_typescript(code, ToolDisclosure::Catalog, Some(registry)).await?;
//!   Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! 1. **Tool Definition**: Tools are defined with JSON Schemas for inputs/outputs
//! 2. **Disclosure Mode**: [`config::ToolDisclosure`] determines how tools are surfaced and the
//!    TypeScript code generation strategy used
//! 3. **Code Generation**: TypeScript interface definitions are generated from schemas; `Catalog`/
//!    `Filesystem` modes emit full namespace implementations, `Sidecar` emits an `InvokeMap`
//! 4. **Code Execution**: User code is wrapped with generated bindings and executed in Deno
//! 5. **Call Routing**: TypeScript function calls are dispatched through [`PctxRegistry`] to Rust
//!    callbacks or MCP servers
//! 6. **Result Marshaling**: JSON values are passed between TypeScript and Rust
//!
//! ## Sandbox Security
//!
//! Code is executed in a Deno runtime with:
//! - Network access restricted to allowed hosts (from registered MCP servers)
//! - No file system access (use `execute_bash` with the virtual filesystem instead)
//! - No subprocess spawning
//! - Isolated V8 context per execution

mod code_mode;
pub mod descriptions;
pub mod model;

// Core execution API
pub use code_mode::CodeMode;

// Re-export config, codegen, registry, crates
pub use pctx_codegen as codegen;
pub use pctx_config as config;
pub use pctx_registry as registry;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MCP Connection error: {0}")]
    McpConnection(#[from] pctx_config::server::McpConnectionError),
    #[error("MCP Service error: {0}")]
    McpService(#[from] pctx_config::server::ServiceError),
    #[error("Codegen error: {0}")]
    Codegen(#[from] pctx_codegen::CodegenError),
    #[error("Registry error: {0}")]
    Registry(#[from] registry::RegistryError),
    #[error("Execution error: {0:?}")]
    Execution(#[from] pctx_executor::DenoExecutorError),
    #[error("Error: {0}")]
    Message(String),
}
