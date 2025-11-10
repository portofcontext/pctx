//! Build script for `pctx_runtime`
//!
//! This script generates a V8 snapshot that includes the `pctx_runtime` extension
//! with all its JavaScript code pre-compiled. This snapshot can be loaded by
//! `pctx_executor` for faster startup times.

use std::borrow::Cow;
use std::env;
use std::error::Error as StdError;
use std::path::PathBuf;

use deno_core::OpState;
use deno_core::extension;
use deno_core::snapshot::CreateSnapshotOptions;
use deno_core::snapshot::create_snapshot;
use deno_error::JsErrorClass;
use deno_error::PropertyValue;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MCPServerConfig {
    name: String,
    url: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CallMCPToolArgs {
    name: String,
    tool: String,
    arguments: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
#[error("MCP error: {0}")]
struct McpError(String);

impl JsErrorClass for McpError {
    fn get_class(&self) -> Cow<'static, str> {
        "Error".into()
    }

    fn get_message(&self) -> Cow<'static, str> {
        self.to_string().into()
    }

    fn get_additional_properties(
        &self,
    ) -> Box<dyn Iterator<Item = (Cow<'static, str>, PropertyValue)>> {
        Box::new(std::iter::empty())
    }

    fn get_ref(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }
}

/// Register an MCP server (stub)
#[deno_core::op2]
#[serde]
fn op_register_mcp(_state: &mut OpState, #[serde] _config: MCPServerConfig) {}

/// Call an MCP tool (async stub)
#[deno_core::op2(async)]
#[serde]
#[allow(clippy::unused_async)]
async fn op_call_mcp_tool(#[serde] _args: CallMCPToolArgs) -> Result<serde_json::Value, McpError> {
    Ok(serde_json::Value::Null)
}

/// Check if an MCP server is registered (stub)
#[deno_core::op2(fast)]
fn op_mcp_has(_state: &mut OpState, #[string] _name: String) -> bool {
    false
}

/// Get an MCP server configuration (stub)
#[deno_core::op2]
#[serde]
fn op_mcp_get(_state: &mut OpState, #[string] _name: String) -> Option<MCPServerConfig> {
    None
}

/// Delete an MCP server configuration (stub)
#[deno_core::op2(fast)]
fn op_mcp_delete(_state: &mut OpState, #[string] _name: String) -> bool {
    false
}

/// Clear all MCP server configurations (stub)
#[deno_core::op2(fast)]
fn op_mcp_clear(_state: &mut OpState) {}

/// Fetch (stub)
#[deno_core::op2(async)]
#[serde]
#[allow(clippy::unused_async)]
async fn op_fetch(
    #[string] _url: String,
    #[serde] _options: Option<serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    Ok(serde_json::Value::Null)
}

// We need to define the extension here as well for snapshot creation
// The esm_entry_point tells deno_core to execute this module during snapshot creation
extension!(
    pctx_runtime_snapshot,
    ops = [
        // Op declarations - these will be registered but not executed during snapshot
        op_register_mcp,
        op_call_mcp_tool,
        op_mcp_has,
        op_mcp_get,
        op_mcp_delete,
        op_mcp_clear,
        op_fetch,
    ],
    esm_entry_point = "ext:pctx_runtime_snapshot/runtime.js",
    esm = [ dir "src", "runtime.js" ],
);

fn main() {
    // Tell cargo to rerun this build script if runtime.js changes
    println!("cargo:rerun-if-changed=src/runtime.js");

    // Get the output directory
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("PCTX_RUNTIME_SNAPSHOT.bin");

    // Create the snapshot
    let snapshot = create_snapshot(
        CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            startup_snapshot: None,
            skip_op_registration: false,
            extensions: vec![pctx_runtime_snapshot::init()],
            extension_transpiler: None,
            with_runtime_cb: None,
        },
        None, // No warmup script
    )
    .expect("Failed to create snapshot");

    // Write the snapshot to disk
    std::fs::write(&snapshot_path, snapshot.output).expect("Failed to write snapshot");

    println!(
        "cargo:rustc-env=PCTX_RUNTIME_SNAPSHOT={}",
        snapshot_path.display()
    );
    println!("Snapshot created at: {}", snapshot_path.display());
}
