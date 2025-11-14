use super::serial;
use crate::execute;

#[tokio::test]
#[serial]
async fn test_execute_simple_code() {
    let code = r"
const x = 1 + 1;
export default x;
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Simple code should execute successfully");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
    assert!(result.diagnostics.is_empty(), "Should have no type errors");
}

#[tokio::test]
#[serial]
async fn test_execute_runtime_error() {
    let code = r#"
throw new Error("This is a runtime error");
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Code with runtime error should fail");
    assert!(result.runtime_error.is_some(), "Should have runtime error");

    let error = result.runtime_error.unwrap();
    assert!(
        error.message.contains("runtime error"),
        "Error should contain the thrown message"
    );
}

#[tokio::test]
#[serial]
async fn test_execute_syntax_error() {
    let code = r"
const x = ;
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Code with syntax error should fail");
    // Syntax errors are caught during execution
    assert!(
        result.runtime_error.is_some() || !result.diagnostics.is_empty(),
        "Should have error information"
    );
}
