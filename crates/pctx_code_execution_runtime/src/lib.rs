//! # PCTX Runtime
//!
//! A Deno extension providing MCP (Model Context Protocol) client functionality and console output capturing.
//!
//! ## Overview
//!
//! This crate provides a pre-compiled V8 snapshot containing:
//! - **MCP Client API**: Register and call MCP tools from JavaScript
//! - **Network Fetch**: Host-permission-based fetch with security controls
//! - **Console Capturing**: Automatic stdout/stderr capture for testing and logging
//!
//! The runtime is designed to be embedded in Deno-based JavaScript execution environments,
//! providing a secure sandbox with controlled access to external services.
//!
//! ## Features
//!
//! - **MCP Integration**: Full Model Context Protocol client with server registry
//! - **Permission System**: Host-based network access controls for fetch operations
//! - **Output Capturing**: Automatic console.log/error capture to buffers
//! - **V8 Snapshot**: Pre-compiled runtime for instant startup
//! - **Type Safety**: Full TypeScript type definitions included
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use deno_core::{JsRuntime, RuntimeOptions};
//! use pctx_code_execution_runtime::{pctx_runtime_snapshot, MCPRegistry, AllowedHosts, RUNTIME_SNAPSHOT};
//! use std::rc::Rc;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new runtime with the PCTX extension
//! let registry = MCPRegistry::new();
//! let allowed_hosts = AllowedHosts::new(Some(vec!["example.com".to_string()]));
//!
//! let mut runtime = JsRuntime::new(RuntimeOptions {
//!     startup_snapshot: Some(RUNTIME_SNAPSHOT),
//!     extensions: vec![pctx_runtime_snapshot::init(registry, allowed_hosts)],
//!     ..Default::default()
//! });
//!
//! // MCP API is now available in JavaScript
//! let code = r#"
//!     registerMCP({ name: "my-server", url: "http://localhost:3000" });
//!     const result = await callMCPTool({
//!         name: "my-server",
//!         tool: "get_data",
//!         arguments: { id: 42 }
//!     });
//!     console.log("Result:", result);
//! "#;
//!
//! runtime.execute_script("<main>", code)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## MCP API
//!
//! The runtime exposes the following global functions to JavaScript:
//!
//! - `registerMCP(config)` - Register an MCP server
//! - `callMCPTool(call)` - Call a tool on a registered server
//! - `REGISTRY.has(name)` - Check if a server is registered
//! - `REGISTRY.get(name)` - Get server configuration
//! - `REGISTRY.delete(name)` - Remove a server
//! - `REGISTRY.clear()` - Remove all servers
//! - `fetch(url, options)` - Fetch with host permission checks
//!
//! ## Console Capturing
//!
//! All `console.log()` and `console.error()` calls are automatically captured:
//!
//! ```javascript
//! console.log("Hello", "World");  // Captured to globalThis.__stdout
//! console.error("Error!");        // Captured to globalThis.__stderr
//! ```
//!
//! ## Security
//!
//! - Network access is controlled via `AllowedHosts` whitelist
//! - Each runtime instance has its own isolated MCP registry
//! - No file system access is provided by default
//!
//! ## Performance
//!
//! - **Startup**: Instant (V8 snapshot pre-compiled)
//! - **Memory**: ~2MB base runtime overhead
//! - **Operations**: Rust ops provide native performance

mod error;
mod fetch;
mod js_error_impl;
pub mod ops;
mod registry;

#[cfg(test)]
mod tests;

pub use fetch::AllowedHosts;
pub use registry::MCPRegistry;

/// Pre-compiled V8 snapshot containing the PCTX runtime
///
/// This snapshot includes:
/// - MCP client JavaScript API (registerMCP, callMCPTool, REGISTRY)
/// - Console output capturing setup
/// - Network fetch with host permissions
/// - TypeScript type definitions
///
/// The snapshot is created at build time and loads instantly at runtime.
///
/// # Example
///
/// ```rust,no_run
/// use deno_core::{JsRuntime, RuntimeOptions};
/// use pctx_code_execution_runtime::{RUNTIME_SNAPSHOT, pctx_runtime_snapshot, MCPRegistry, AllowedHosts};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let registry = MCPRegistry::new();
/// let allowed_hosts = AllowedHosts::new(None);
///
/// let mut runtime = JsRuntime::new(RuntimeOptions {
///     startup_snapshot: Some(RUNTIME_SNAPSHOT),
///     extensions: vec![pctx_runtime_snapshot::init(registry, allowed_hosts)],
///     ..Default::default()
/// });
/// # Ok(())
/// # }
/// ```
pub static RUNTIME_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/PCTX_RUNTIME_SNAPSHOT.bin"));

// Deno extension providing MCP client and console capturing.
// Initialize with MCPRegistry and AllowedHosts configuration.
// See README.md for complete documentation.
deno_core::extension!(
    pctx_runtime_snapshot,
    ops = [
        ops::op_register_mcp,
        ops::op_call_mcp_tool,
        ops::op_mcp_has,
        ops::op_mcp_get,
        ops::op_mcp_delete,
        ops::op_mcp_clear,
        ops::op_fetch,
    ],
    esm_entry_point = "ext:pctx_runtime_snapshot/runtime.js",
    esm = [ dir "src", "runtime.js" ],
    options = {
        registry: MCPRegistry,
        allowed_hosts: AllowedHosts,
    },
    state = |state, options| {
        state.put(options.registry);
        state.put(options.allowed_hosts);
    },
);
