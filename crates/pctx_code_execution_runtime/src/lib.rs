// Allow long const eval for large JavaScript bundles
#![allow(long_running_const_eval)]

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

mod callback_ops;
mod callback_registry;
mod error;
mod js_error_impl;
pub mod mcp_ops;
mod mcp_registry;
mod timer_ops;

pub use callback_registry::{CallbackFn, CallbackRegistry};
pub use mcp_registry::MCPRegistry;

/// Pre-compiled V8 snapshot containing the PCTX runtime
///
/// This snapshot includes:
/// - MCP tool calling JavaScript API (callMCPTool)
/// - Callback calling JavaScript API (invokeCallback)
/// - Console output capturing setup
/// - Network fetch with host permissions
/// - TypeScript type definitions
///
/// The snapshot is created at build time and loads instantly at runtime.
pub static RUNTIME_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/PCTX_RUNTIME_SNAPSHOT.bin"));

// Deno extension providing MCP client, local tools, and console capturing.
// Initialize with MCPRegistry, CallableToolRegistry, and AllowedHosts configuration.
// See README.md for complete documentation.
deno_core::extension!(
    pctx_runtime_snapshot,
    ops = [
        mcp_ops::op_call_mcp_tool,
        callback_ops::op_invoke_callback,
        timer_ops::op_sleep,
    ],
    esm_entry_point = "ext:pctx_runtime_snapshot/runtime.js",
    esm = [
        dir "src",
        "timers.js",
        "runtime.js",
        "just-bash/bundle.js",
        "just-bash/node_zlib_stub.js",
    ],
    options = {
        registry: MCPRegistry,
        callback_registry: CallbackRegistry,
    },
    state = |state, options| {
        state.put(options.registry);
        state.put(options.callback_registry);
    },
);
