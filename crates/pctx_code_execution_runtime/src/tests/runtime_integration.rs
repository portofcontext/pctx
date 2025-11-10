//! Integration tests that spin up a JavaScript runtime
//!
//! These tests verify that the MCP client works correctly when accessed from JavaScript

use crate::mcp_client::MCPRegistry;
use deno_core::{JsRuntime, PollEventLoopOptions, RuntimeOptions, op2};
use serde_json::json;

// Custom op to capture test results
#[op2]
#[serde]
fn op_test_set_result(#[serde] value: serde_json::Value) -> serde_json::Value {
    value
}

/// Helper function to create a JavaScript runtime with `pctx_runtime` extension and test ops
fn create_test_runtime() -> JsRuntime {
    let registry = MCPRegistry::new();
    let allowed_hosts = crate::AllowedHosts::default();

    // Create a simple extension for test helpers
    deno_core::extension!(test_helpers, ops = [op_test_set_result],);

    JsRuntime::new(RuntimeOptions {
        extensions: vec![
            test_helpers::init(),
            crate::pctx_runtime_snapshot::init(registry, allowed_hosts),
        ],
        ..Default::default()
    })
}

/// Helper function to execute JavaScript code and get the result as JSON
async fn execute_js(code: &str) -> Result<serde_json::Value, String> {
    let mut runtime = create_test_runtime();

    // Inject test helper
    runtime
        .execute_script(
            "<inject_helper>",
            r"
            globalThis.setTestResult = (val) => {
                return Deno.core.ops.op_test_set_result(val);
            };
            ",
        )
        .map_err(|e| format!("Failed to inject helper: {e}"))?;

    // Wrap the code to call setTestResult
    // Note: The code should NOT use import statements - use the global APIs instead
    // The runtime.js already exposes registerMCP, callMCPTool, and REGISTRY globally
    let wrapped_code = format!(
        r"
        (async () => {{
            const result = await (async () => {{
                {code}
            }})();
            return setTestResult(result);
        }})();
        "
    );

    // Execute the code and get the promise
    let promise = runtime
        .execute_script("<test>", wrapped_code)
        .map_err(|e| format!("Script execution failed: {e}"))?;

    // Resolve the promise first
    let resolve_future = runtime.resolve(promise);

    // Then run it with the event loop
    let resolved = runtime
        .with_event_loop_promise(resolve_future, PollEventLoopOptions::default())
        .await
        .map_err(|e| format!("Failed to resolve promise: {e}"))?;

    // Convert the resolved value to JSON
    let json_value = {
        deno_core::scope!(scope, &mut runtime);
        let local = deno_core::v8::Local::new(scope, resolved);
        deno_core::serde_v8::from_v8::<serde_json::Value>(scope, local)
            .map_err(|e| format!("Failed to convert result to JSON: {e}"))?
    };

    Ok(json_value)
}

#[tokio::test]
async fn test_runtime_register_mcp() {
    let code = r#"
        registerMCP({
            name: "test-server",
            url: "http://localhost:3000"
        });

        return REGISTRY.has("test-server");
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    assert_eq!(result, json!(true), "Server should be registered");
}

#[tokio::test]
async fn test_runtime_register_mcp_global_api() {
    let code = r#"
        registerMCP({
            name: "global-test-server",
            url: "http://localhost:4000"
        });

        return REGISTRY.has("global-test-server");
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    assert_eq!(
        result,
        json!(true),
        "Server should be registered via global API"
    );
}

#[tokio::test]
async fn test_runtime_duplicate_registration_throws() {
    let code = r#"
        registerMCP({
            name: "duplicate-server",
            url: "http://localhost:3000"
        });

        try {
            registerMCP({
                name: "duplicate-server",
                url: "http://localhost:3001"
            });
            return false;
        } catch (e) {
            return e.message.includes("already registered");
        }
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    assert_eq!(
        result,
        json!(true),
        "Should catch duplicate registration error"
    );
}

#[tokio::test]
async fn test_runtime_get_config() {
    let code = r#"
        registerMCP({
            name: "my-server",
            url: "http://localhost:5000"
        });

        const config = REGISTRY.get("my-server");
        return config;
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    let obj = result.as_object().expect("Should be an object");
    assert_eq!(obj.get("name").unwrap(), "my-server");
    assert_eq!(obj.get("url").unwrap(), "http://localhost:5000");
}

#[tokio::test]
async fn test_runtime_get_nonexistent() {
    let code = r#"
        const config = REGISTRY.get("nonexistent");
        return config === undefined ? null : config;
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    assert!(
        result.is_null(),
        "Nonexistent server should return undefined/null"
    );
}

#[tokio::test]
async fn test_runtime_delete_server() {
    let code = r#"
        registerMCP({
            name: "temp-server",
            url: "http://localhost:6000"
        });

        const hasBefore = REGISTRY.has("temp-server");
        const deleted = REGISTRY.delete("temp-server");
        const hasAfter = REGISTRY.has("temp-server");

        return { hasBefore, deleted, hasAfter };
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    assert_eq!(
        result,
        json!({
            "hasBefore": true,
            "deleted": true,
            "hasAfter": false
        }),
        "Server should be deleted successfully"
    );
}

#[tokio::test]
async fn test_runtime_clear_all_servers() {
    let code = r#"
        registerMCP({ name: "server1", url: "http://localhost:3001" });
        registerMCP({ name: "server2", url: "http://localhost:3002" });
        registerMCP({ name: "server3", url: "http://localhost:3003" });

        const countBefore = REGISTRY.has("server1") && REGISTRY.has("server2") && REGISTRY.has("server3");

        REGISTRY.clear();

        const countAfter = REGISTRY.has("server1") || REGISTRY.has("server2") || REGISTRY.has("server3");

        return { countBefore, countAfter };
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    assert_eq!(
        result,
        json!({
            "countBefore": true,
            "countAfter": false
        }),
        "All servers should be cleared"
    );
}

#[tokio::test]
async fn test_runtime_multiple_servers() {
    let code = r#"
        const servers = [
            { name: "server1", url: "http://localhost:3001" },
            { name: "server2", url: "http://localhost:3002" },
            { name: "server3", url: "http://localhost:3003" },
            { name: "server4", url: "http://localhost:3004" },
        ];

        servers.forEach(s => registerMCP(s));

        const results = servers.map(s => ({
            name: s.name,
            exists: REGISTRY.has(s.name),
            config: REGISTRY.get(s.name)
        }));

        return results;
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    let servers = result.as_array().expect("Should be an array");
    assert_eq!(servers.len(), 4, "Should have 4 servers");

    for server in servers {
        let obj = server.as_object().expect("Each result should be an object");
        assert_eq!(
            obj.get("exists").unwrap(),
            &json!(true),
            "Server should exist"
        );
        assert!(obj.get("config").is_some(), "Config should be present");
    }
}

#[tokio::test]
async fn test_runtime_console_output_capturing() {
    let code = r#"
        console.log("test log message");
        console.error("test error message");
        console.warn("test warning");
        console.info("test info");

        return {
            stdout: globalThis.__stdout,
            stderr: globalThis.__stderr
        };
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    let obj = result.as_object().expect("Should be an object");

    let stdout = obj
        .get("stdout")
        .unwrap()
        .as_array()
        .expect("stdout should be array");
    let stderr = obj
        .get("stderr")
        .unwrap()
        .as_array()
        .expect("stderr should be array");

    // Check that console.log and console.info went to stdout
    assert!(
        stdout
            .iter()
            .any(|v| v.as_str().unwrap().contains("test log message")),
        "stdout should contain log message"
    );
    assert!(
        stdout
            .iter()
            .any(|v| v.as_str().unwrap().contains("test info")),
        "stdout should contain info message"
    );

    // Check that console.error and console.warn went to stderr
    assert!(
        stderr
            .iter()
            .any(|v| v.as_str().unwrap().contains("test error message")),
        "stderr should contain error message"
    );
    assert!(
        stderr
            .iter()
            .any(|v| v.as_str().unwrap().contains("test warning")),
        "stderr should contain warning"
    );
}

#[tokio::test]
async fn test_runtime_console_captures_objects() {
    let code = r#"
        console.log({ foo: "bar", num: 42 });
        console.log(null);
        console.log(undefined);

        return globalThis.__stdout;
    "#;

    let result = execute_js(code).await.expect("Should execute successfully");
    let stdout = result.as_array().expect("Should be an array");

    // Objects should be JSON stringified
    let obj_msg = stdout[0].as_str().unwrap();
    assert!(obj_msg.contains("foo") && obj_msg.contains("bar") && obj_msg.contains("42"));

    // null should be "null"
    assert_eq!(stdout[1].as_str().unwrap(), "null");

    // undefined should be "undefined"
    assert_eq!(stdout[2].as_str().unwrap(), "undefined");
}
