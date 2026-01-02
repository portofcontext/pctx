//! Build script for `pctx_runtime`
//!
//! This script generates a V8 snapshot that includes the `pctx_runtime` extension
//! with all its JavaScript code pre-compiled. This snapshot can be loaded by
//! `pctx_executor` for faster startup times.

use std::env;
use std::path::PathBuf;

use deno_core::extension;
use deno_core::snapshot::CreateSnapshotOptions;
use deno_core::snapshot::create_snapshot;

use rmcp::model::JsonObject;

/// Call an MCP tool (async stub)
#[deno_core::op2(async)]
#[serde]
#[allow(clippy::unused_async)]
async fn op_call_mcp_tool(
    #[string] _server_name: String,
    #[string] _tool_name: String,
    #[serde] _args: Option<JsonObject>,
) -> serde_json::Value {
    serde_json::Value::Null
}

/// Invoke callback (stub)
#[deno_core::op2(async)]
#[serde]
#[allow(clippy::unused_async)]
async fn op_invoke_callback(
    #[string] _id: String,
    #[serde] _arguments: Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::Value::Null
}

/// Fetch (stub)
#[deno_core::op2(async)]
#[serde]
#[allow(clippy::unused_async)]
async fn op_fetch(
    #[string] _url: String,
    #[serde] _options: Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::Value::Null
}

// We need to define the extension here as well for snapshot creation
// The esm_entry_point tells deno_core to execute this module during snapshot creation
extension!(
    pctx_runtime_snapshot,
    ops = [
        // Op declarations - these will be registered but not executed during snapshot
        op_call_mcp_tool,
        op_invoke_callback,
        op_fetch,
    ],
    esm_entry_point = "ext:pctx_runtime_snapshot/runtime.js",
    esm = [ dir "src", "runtime.js" ],
);

fn main() {
    // Tell cargo to rerun this build script if runtime.js or build.rs changes
    println!("cargo:rerun-if-changed=src/runtime.js");
    println!("cargo:rerun-if-changed=build.rs");

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
