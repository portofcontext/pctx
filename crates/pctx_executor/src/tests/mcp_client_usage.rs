use super::serial;
use crate::{ExecuteOptions, execute};
use pctx_config::server::ServerConfig;
use serde_json::json;
use url::Url;

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_duplicate_registration() {
    let code = r"
export default true;
";

    // Attempt to register the same server twice
    let mcp_configs = vec![
        ServerConfig::new(
            "duplicate-server".to_string(),
            Url::parse("http://localhost:3000").unwrap(),
        ),
        ServerConfig::new(
            "duplicate-server".to_string(),
            Url::parse("http://localhost:3001").unwrap(),
        ),
    ];

    let result = execute(code, ExecuteOptions::new().with_servers(mcp_configs))
        .await
        .expect("execution should succeed");
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
async fn test_execute_with_mcp_client_call_tool_nonexistent_server() {
    let code = r#"

async function test() {
    try {
        await callMCPTool({
            serverName: "nonexistent-server",
            toolName: "some-tool"
        });
        return { error: false };
    } catch (e) {
        return { error: true, message: e.message };
    }
}

export default await test();
"#;

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");

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
        message.contains("nonexistent-server"),
        "Error message should mention nonexistent server, got: {message}"
    );
}
