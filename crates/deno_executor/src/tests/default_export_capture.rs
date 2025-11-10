use crate::*;

#[tokio::test]
async fn test_capture_simple_number_export() {
    let code = r"
const x: number = 1 + 1;
export default x;
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");
    assert_eq!(
        result.output.unwrap(),
        serde_json::json!(2),
        "Should capture the number value"
    );
}

#[tokio::test]
async fn test_capture_string_export() {
    let code = r#"
const greeting = "Hello, World!";
export default greeting;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");
    assert_eq!(
        result.output.unwrap(),
        serde_json::json!("Hello, World!"),
        "Should capture the string value"
    );
}

#[tokio::test]
async fn test_capture_object_export() {
    let code = r#"
const data = { name: "Alice", age: 30 };
export default data;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");

    let output = result.output.unwrap();
    assert_eq!(output["name"], "Alice");
    assert_eq!(output["age"], 30);
}

#[tokio::test]
async fn test_capture_array_export() {
    let code = r"
const numbers = [1, 2, 3, 4, 5];
export default numbers;
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");
    assert_eq!(
        result.output.unwrap(),
        serde_json::json!([1, 2, 3, 4, 5]),
        "Should capture the array value"
    );
}

#[tokio::test]
async fn test_no_default_export() {
    let code = r"
const x = 42;
console.log(x);
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(
        result.output.is_none(),
        "Should have no output when no default export"
    );
    assert!(
        result.stdout.contains("42"),
        "Should still capture console output"
    );
}

#[tokio::test]
async fn test_capture_with_console_output() {
    let code = r#"
console.log("Processing...");
const result = { value: 42 };
console.log("Done!");
export default result;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");
    assert_eq!(
        result.output.unwrap()["value"],
        42,
        "Should capture the exported value"
    );
    assert!(
        result.stdout.contains("Processing..."),
        "Should capture console output"
    );
    assert!(
        result.stdout.contains("Done!"),
        "Should capture all console output"
    );
}

#[tokio::test]
async fn test_capture_boolean_export() {
    let code = r"
const isValid = true;
export default isValid;
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");
    assert_eq!(
        result.output.unwrap(),
        serde_json::json!(true),
        "Should capture boolean value"
    );
}

#[tokio::test]
async fn test_capture_null_export() {
    let code = r"
export default null;
";

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture null export");
    assert_eq!(
        result.output.unwrap(),
        serde_json::json!(null),
        "Should capture null value"
    );
}

#[tokio::test]
async fn test_no_output_on_type_error() {
    let code = r#"
const x: number = "string";
export default x;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Type error should cause failure");
    assert!(
        result.output.is_none(),
        "Should have no output on type error"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "Should have type diagnostics"
    );
}

#[tokio::test]
async fn test_no_output_on_runtime_error() {
    let code = r#"
throw new Error("Runtime error");
export default 42;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Runtime error should cause failure");
    assert!(
        result.output.is_none(),
        "Should have no output on runtime error"
    );
    assert!(result.runtime_error.is_some(), "Should have runtime error");
}

#[tokio::test]
async fn test_capture_nested_object() {
    let code = r#"
const data = {
    user: {
        name: "Alice",
        profile: {
            age: 30,
            email: "alice@example.com"
        }
    },
    settings: {
        theme: "dark",
        notifications: true
    }
};
export default data;
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Code should execute successfully");
    assert!(result.output.is_some(), "Should capture default export");

    let output = result.output.unwrap();
    assert_eq!(output["user"]["name"], "Alice");
    assert_eq!(output["user"]["profile"]["age"], 30);
    assert_eq!(output["settings"]["theme"], "dark");
}
