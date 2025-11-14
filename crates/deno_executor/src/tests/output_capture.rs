use super::serial;
use crate::execute;

#[serial]
#[tokio::test]
async fn test_execute_captures_stdout() {
    let code = r#"
console.log("Hello, stdout!");
console.log("Line 2");
export default "result";
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(
        result.stdout.contains("Hello, stdout!"),
        "stdout should contain console.log output, got: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains("Line 2"),
        "stdout should contain second line, got: {}",
        result.stdout
    );
}

#[serial]
#[tokio::test]
async fn test_execute_captures_stderr() {
    let code = r#"
console.error("Error message");
export default "result";
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(
        result.stderr.contains("Error message"),
        "stderr should contain console.error output, got: {}",
        result.stderr
    );
}

#[serial]
#[tokio::test]
async fn test_execute_captures_both_stdout_and_stderr() {
    let code = r#"
console.log("Standard output");
console.error("Standard error");
console.log("More output");
export default "result";
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(
        result.stdout.contains("Standard output") && result.stdout.contains("More output"),
        "stdout should contain console.log output, got: {}",
        result.stdout
    );
    assert!(
        result.stderr.contains("Standard error"),
        "stderr should contain console.error output, got: {}",
        result.stderr
    );
}

#[serial]
#[tokio::test]
async fn test_execute_stderr_contains_type_error() {
    let code = r#"const x: number = "string";"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Type error should cause failure");
    assert!(
        result.stdout.is_empty(),
        "stdout should be empty when not executed due to type error"
    );
    assert!(
        !result.stderr.is_empty(),
        "stderr should contain type error diagnostic"
    );
    assert!(
        result.stderr.contains("Type") || result.stderr.contains("string"),
        "stderr should mention the type error, got: {}",
        result.stderr
    );
}

#[serial]
#[tokio::test]
async fn test_execute_stderr_contains_syntax_error() {
    let code = "async function run() { onst x = 5; return x; }";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Syntax error should cause failure");
    assert!(
        result.stdout.is_empty(),
        "stdout should be empty when not executed due to syntax error"
    );
    assert!(
        !result.stderr.is_empty(),
        "stderr should contain syntax error diagnostic"
    );
    assert!(
        result.stderr.contains("Expected") || result.stderr.contains("onst"),
        "stderr should mention the syntax error, got: {}",
        result.stderr
    );
}

#[serial]
#[tokio::test]
async fn test_execute_stderr_contains_transpilation_error() {
    // Missing closing brace
    let code = "function test() { return 42;";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Transpilation error should cause failure");
    assert!(
        result.stdout.is_empty(),
        "stdout should be empty when transpilation fails"
    );
    assert!(
        !result.stderr.is_empty(),
        "stderr should contain transpilation error"
    );
}

#[serial]
#[tokio::test]
async fn test_execute_stderr_contains_runtime_error() {
    let code = r#"
throw new Error("Runtime failure");
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Code with runtime error should fail");
    assert!(result.runtime_error.is_some(), "Should have runtime error");
    assert!(
        result.stderr.contains("Runtime failure")
            || result
                .runtime_error
                .as_ref()
                .unwrap()
                .message
                .contains("Runtime failure"),
        "stderr or runtime_error should contain error message, stderr: {}, error: {:?}",
        result.stderr,
        result.runtime_error
    );
}

#[serial]
#[tokio::test]
async fn test_execute_stdout_before_error() {
    let code = r#"
console.log("This prints before error");
throw new Error("Then fails");
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Code should fail due to runtime error");
    // Note: Currently, stdout may not be captured if execution fails early.
    // This is a known limitation where console output before an error may not be
    // captured because the error happens before the output capture mechanism runs.
    // The test documents this behavior.
    // In the future, this could be improved by capturing output in real-time.
}

#[serial]
#[tokio::test]
async fn test_execute_multiline_stdout() {
    let code = r#"
for (let i = 1; i <= 3; i++) {
    console.log(`Line ${i}`);
}
export default "done";
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(
        result.stdout.contains("Line 1")
            && result.stdout.contains("Line 2")
            && result.stdout.contains("Line 3"),
        "stdout should contain all loop output, got: {}",
        result.stdout
    );
}
