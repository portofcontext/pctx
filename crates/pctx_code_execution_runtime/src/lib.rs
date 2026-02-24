// Allow long const eval for large JavaScript bundles
#![allow(long_running_const_eval)]

//! # PCTX Runtime
//!
//! A Deno extension providing registry-backed tool invocation and console output capturing.
//!
//! ## Overview
//!
//! This crate provides a pre-compiled V8 snapshot containing:
//! - **Registry Invocation**: Call any registered action (local callbacks, MCP tools, etc.) from JavaScript
//! - **Console Capturing**: Automatic stdout/stderr capture for testing and logging
//! - **Bash Execution**: `justBash` global for running shell commands via `just-bash`
//! - **Timers**: `setTimeout`/`setInterval` support
//!
//! The extension is initialized with a `PctxRegistry` that routes `invokeInternal` calls
//! to the appropriate handler at runtime.
//!
//! ## JavaScript API
//!
//! The runtime exposes the following globals:
//!
//! - `invokeInternal({ name, arguments })` - Invoke a registered action by ID
//! - `justBash` - Shell execution via the `just-bash` library
//!
//! ## Console Capturing
//!
//! All console methods are overridden and captured to arrays:
//!
//! ```javascript
//! console.log("hello");   // -> globalThis.__stdout
//! console.error("oops");  // -> globalThis.__stderr
//! console.warn("warn");   // -> globalThis.__stderr
//! console.info("info");   // -> globalThis.__stdout
//! console.debug("dbg");   // -> globalThis.__stdout
//! ```

mod invoke_ops;
mod timer_ops;

pub use pctx_registry::{CallbackFn, PctxRegistry, RegistryError};

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
        invoke_ops::op_invoke,
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
        registry: PctxRegistry,
    },
    state = |state, options| {
        state.put(options.registry);
    },
);
