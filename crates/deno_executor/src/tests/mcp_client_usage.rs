use super::serial;
use crate::execute;
use serde_json::json;

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_register() {
    let code = r#"

registerMCP({
    name: "test-server",
    url: "http://localhost:3000"
});

const registered = REGISTRY.has("test-server");
console.log("registered value:", registered);

export default registered;
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        result.success,
        "MCP client registration should succeed. Error: {:?}",
        result.runtime_error
    );
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
    assert!(result.diagnostics.is_empty(), "Should have no type errors");

    // Assert actual output value
    assert_eq!(
        result.output,
        Some(json!(true)),
        "Should return true when server is registered"
    );
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_duplicate_registration() {
    let code = r#"

registerMCP({
    name: "duplicate-server",
    url: "http://localhost:3000"
});

// This should throw an error
registerMCP({
    name: "duplicate-server",
    url: "http://localhost:3001"
});

export default true;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Duplicate MCP registration should fail");
    assert!(result.runtime_error.is_some(), "Should have runtime error");

    let error = result.runtime_error.unwrap();
    assert!(
        error.message.contains("already registered") || error.message.contains("duplicate"),
        "Error should mention duplicate registration, got: {}",
        error.message
    );
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_get_config() {
    let code = r#"

registerMCP({
    name: "my-server",
    url: "http://localhost:4000"
});

const config = REGISTRY.get("my-server");

export default config;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Getting MCP config should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );

    // Assert actual output value contains correct config
    let output = result.output.expect("Should have output");
    let config = output.as_object().expect("Should be an object");
    assert_eq!(config.get("name").unwrap(), "my-server");
    assert_eq!(config.get("url").unwrap(), "http://localhost:4000/");
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_multiple_servers() {
    let code = r#"

registerMCP({
    name: "server1",
    url: "http://localhost:3000"
});

registerMCP({
    name: "server2",
    url: "http://localhost:3001"
});

registerMCP({
    name: "server3",
    url: "http://localhost:3002"
});

const hasServer1 = REGISTRY.has("server1");
const hasServer2 = REGISTRY.has("server2");
const hasServer3 = REGISTRY.has("server3");

export default { hasServer1, hasServer2, hasServer3 };
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(
        result.success,
        "Multiple server registration should succeed"
    );
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );

    // Assert actual output values
    assert_eq!(
        result.output,
        Some(json!({
            "hasServer1": true,
            "hasServer2": true,
            "hasServer3": true
        })),
        "All three servers should be registered"
    );
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_registry_operations() {
    let code = r#"

registerMCP({
    name: "temp-server",
    url: "http://localhost:5000"
});

const existsBefore = REGISTRY.has("temp-server");
REGISTRY.delete("temp-server");
const existsAfter = REGISTRY.has("temp-server");

export default { existsBefore, existsAfter };
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Registry operations should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );

    // Assert actual output values
    assert_eq!(
        result.output,
        Some(json!({
            "existsBefore": true,
            "existsAfter": false
        })),
        "Server should exist before delete and not exist after"
    );
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_registry_clear() {
    let code = r#"

registerMCP({
    name: "server1",
    url: "http://localhost:3000"
});

registerMCP({
    name: "server2",
    url: "http://localhost:3001"
});

const hasBefore = REGISTRY.has("server1") && REGISTRY.has("server2");
REGISTRY.clear();
const hasAfter = REGISTRY.has("server1") || REGISTRY.has("server2");

export default { hasBefore, hasAfter };
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Registry clear should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );

    // Assert actual output values
    assert_eq!(
        result.output,
        Some(json!({
            "hasBefore": true,
            "hasAfter": false
        })),
        "All servers should exist before clear and none after"
    );
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_delete_nonexistent() {
    let code = r#"

const deleteResult = REGISTRY.delete("nonexistent-server");

export default deleteResult;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Deleting nonexistent server should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );

    // Assert actual output value
    assert_eq!(
        result.output,
        Some(json!(false)),
        "Delete should return false for nonexistent server"
    );
}

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_call_tool_nonexistent_server() {
    let code = r#"

async function test() {
    try {
        await callMCPTool({
            name: "nonexistent-server",
            tool: "some-tool"
        });
        return { error: false };
    } catch (e) {
        return { error: true, message: e.message };
    }
}

export default await test();
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Execution should succeed even with error");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors (error was caught)"
    );

    // Assert actual output values
    let output = result.output.expect("Should have output");
    let obj = output.as_object().expect("Should be an object");
    assert_eq!(
        obj.get("error").unwrap(),
        &json!(true),
        "Should have caught error"
    );
    let message = obj.get("message").unwrap().as_str().unwrap();
    assert!(
        message.contains("does not exist") || message.contains("nonexistent-server"),
        "Error message should mention nonexistent server, got: {message}"
    );
}
