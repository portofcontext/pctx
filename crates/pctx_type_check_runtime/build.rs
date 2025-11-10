//! Build script for `pctx_type_check`
//!
//! This script generates a V8 snapshot that includes the TypeScript compiler
//! for full type checking capabilities.

#![allow(long_running_const_eval)]

use std::env;
use std::path::PathBuf;

use deno_core::extension;
use deno_core::snapshot::CreateSnapshotOptions;
use deno_core::snapshot::create_snapshot;

// Define the extension for snapshot creation
// The esm_entry_point tells deno_core to execute this module during snapshot creation
extension!(
    pctx_type_check_snapshot,
    esm_entry_point = "ext:pctx_type_check_snapshot/type_check_runtime.js",
    esm = [
        dir "src",
        "typescript.min.js",
        "type_check_runtime.js"
    ],
);

fn main() {
    // Tell cargo to rerun this build script if source files change
    println!("cargo:rerun-if-changed=src/typescript.min.js");
    println!("cargo:rerun-if-changed=src/type_check_runtime.js");

    // Get the output directory
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("PCTX_TYPE_CHECK_SNAPSHOT.bin");

    // Create the snapshot
    let snapshot = create_snapshot(
        CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            startup_snapshot: None,
            skip_op_registration: false,
            extensions: vec![pctx_type_check_snapshot::init()],
            extension_transpiler: None,
            with_runtime_cb: None,
        },
        None, // No warmup script
    )
    .expect("Failed to create snapshot");

    // Write the snapshot to disk
    std::fs::write(&snapshot_path, snapshot.output).expect("Failed to write snapshot");

    println!(
        "cargo:rustc-env=PCTX_TYPE_CHECK_SNAPSHOT={}",
        snapshot_path.display()
    );
    println!("Type check snapshot created at: {}", snapshot_path.display());
}
