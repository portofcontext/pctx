use super::serial;
use crate::{ExecuteOptions, execute};
use serde_json::json;

#[serial]
#[tokio::test]
async fn test_execute_with_mcp_client_call_tool_nonexistent_server() {
    let code = r#"

async function test() {
    try {
        await invokeInternal({
            name: "nonexistent-server___some-tool"
        });
        return { error: false };
    } catch (e) {
        return { error: true, message: e instanceof Error ? e.message : String(e) };
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
