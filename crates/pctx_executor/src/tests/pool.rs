/// Integration tests for [`ExecutorPool`].
///
/// These tests spawn real `pctx_worker` sub-processes.  `cargo test -p
/// pctx_executor` builds all binaries in the package (including
/// `pctx_worker`) before running tests, so the binary is always available
/// at `target/{profile}/pctx_worker` — one directory above the test
/// runner binary in `target/{profile}/deps/`.
///
/// Pool tests do NOT need `#[serial]`: the V8 mutex lives inside each
/// worker process, not in the test process.
use std::sync::Arc;

use serde_json::json;

use crate::{ExecuteOptions, ExecutorPool, PoolConfig};
use pctx_registry::PctxRegistry;

/// Locate `pctx_worker` relative to the running test binary.
fn worker_pool(worker_count: usize) -> PoolConfig {
    let test_exe = std::env::current_exe().expect("current_exe");
    // test binary:  .../target/debug/deps/pctx_executor-<hash>
    // worker binary: .../target/debug/pctx_worker
    let bin_dir = test_exe
        .parent() // .../deps
        .and_then(|d| d.parent()) // .../debug  (or release)
        .expect("could not determine bin dir from test exe path");

    PoolConfig {
        worker_count,
        worker_binary: bin_dir.join("pctx_worker"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Basic execution
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pool_executes_simple_code() {
    let pool = ExecutorPool::new(worker_pool(1)).await.expect("pool");
    let result = pool
        .execute("export default 42;", ExecuteOptions::new())
        .await
        .expect("execute");

    assert!(result.success);
    assert_eq!(result.output, Some(json!(42)));
    assert!(result.runtime_error.is_none());
    assert!(result.diagnostics.is_empty());
}

#[tokio::test]
async fn pool_captures_stdout() {
    let pool = ExecutorPool::new(worker_pool(1)).await.expect("pool");
    let result = pool
        .execute(
            r#"console.log("hello from worker"); export default null;"#,
            ExecuteOptions::new(),
        )
        .await
        .expect("execute");

    assert!(result.success);
    assert!(result.stdout.contains("hello from worker"));
}

#[tokio::test]
async fn pool_reports_runtime_error() {
    let pool = ExecutorPool::new(worker_pool(1)).await.expect("pool");
    let result = pool
        .execute(
            r#"throw new Error("boom"); export default null;"#,
            ExecuteOptions::new(),
        )
        .await
        .expect("execute returned Err — expected Ok with runtime_error set");

    assert!(!result.success);
    assert!(result.runtime_error.is_some());
    assert!(
        result
            .runtime_error
            .as_ref()
            .unwrap()
            .message
            .contains("boom")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback proxying — the core new IPC path
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pool_proxies_callback_to_parent() {
    let registry = PctxRegistry::default();
    registry
        .add_callback(
            "math.add",
            Arc::new(|args: Option<serde_json::Value>| {
                Box::pin(async move {
                    let a = args.as_ref().and_then(|v| v["a"].as_i64()).unwrap_or(0);
                    let b = args.as_ref().and_then(|v| v["b"].as_i64()).unwrap_or(0);
                    Ok(json!(a + b))
                })
            }),
        )
        .expect("add_callback");

    let pool = ExecutorPool::new(worker_pool(1)).await.expect("pool");
    let result = pool
        .execute(
            r#"
async function run() {
    return await invokeInternal({ name: "math.add", arguments: { a: 7, b: 3 } });
}
export default await run();
"#,
            ExecuteOptions::new().with_registry(registry),
        )
        .await
        .expect("execute");

    assert!(result.success, "stderr: {}", result.stderr);
    assert_eq!(result.output, Some(json!(10)));
}

#[tokio::test]
async fn pool_proxies_multiple_callback_invocations() {
    let registry = PctxRegistry::default();
    registry
        .add_callback(
            "counter.inc",
            Arc::new(|args: Option<serde_json::Value>| {
                Box::pin(async move {
                    let n = args.as_ref().and_then(|v| v["n"].as_i64()).unwrap_or(0);
                    Ok(json!(n + 1))
                })
            }),
        )
        .expect("add_callback");

    let pool = ExecutorPool::new(worker_pool(1)).await.expect("pool");
    let result = pool
        .execute(
            r#"
async function run() {
    let v = 0;
    for (let i = 0; i < 5; i++) {
        v = await invokeInternal({ name: "counter.inc", arguments: { n: v } });
    }
    return v;
}
export default await run();
"#,
            ExecuteOptions::new().with_registry(registry),
        )
        .await
        .expect("execute");

    assert!(result.success, "stderr: {}", result.stderr);
    assert_eq!(result.output, Some(json!(5)));
}

// ─────────────────────────────────────────────────────────────────────────────
// MCP tool proxying — no direct connections from the worker
// ─────────────────────────────────────────────────────────────────────────────

/// Registers a tool with an MCP-style ID (`"server__tool"`) as a plain Rust
/// callback in the parent registry.  The worker must proxy the call back to
/// the parent rather than attempting a live MCP connection.
///
/// This is the regression test for the fix: previously the worker would try
/// to open a fresh HTTP connection to the MCP server, which fails in any
/// environment that doesn't have access to that host.  Now the worker only
/// holds IPC-proxy stubs and the parent's registry handles the actual dispatch.
#[tokio::test]
async fn pool_proxies_mcp_style_tool_id_through_parent() {
    let registry = PctxRegistry::default();

    // Register with the same `"server__tool"` format the MCP registry uses.
    // The callback simulates what the real MCP server would return.
    registry
        .add_callback(
            "my_server__get_item",
            Arc::new(|args: Option<serde_json::Value>| {
                Box::pin(async move {
                    let id = args.as_ref().and_then(|v| v["id"].as_str()).unwrap_or("?");
                    Ok(json!({ "id": id, "name": "widget" }))
                })
            }),
        )
        .expect("add_callback");

    let pool = ExecutorPool::new(worker_pool(1)).await.expect("pool");
    let result = pool
        .execute(
            r#"
async function run() {
    return await invokeInternal({
        name: "my_server__get_item",
        arguments: { id: "abc123" },
    });
}
export default await run();
"#,
            ExecuteOptions::new().with_registry(registry),
        )
        .await
        .expect("execute");

    assert!(result.success, "stderr: {}", result.stderr);
    assert_eq!(result.output, Some(json!({ "id": "abc123", "name": "widget" })));
}

// ─────────────────────────────────────────────────────────────────────────────
// Round-robin dispatch
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pool_reuses_workers_across_executions() {
    // A 2-worker pool running 6 executions: verifies the pool doesn't break
    // after the first round of requests and workers handle reuse correctly.
    let pool = ExecutorPool::new(worker_pool(2)).await.expect("pool");

    for i in 0i32..6 {
        let result = pool
            .execute(
                &format!("export default {i};"),
                ExecuteOptions::new(),
            )
            .await
            .expect("execute");

        assert!(result.success);
        assert_eq!(result.output, Some(json!(i)));
    }
}
