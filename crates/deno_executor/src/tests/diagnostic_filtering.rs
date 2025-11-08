use crate::*;
/// Tests that we ignore typescript errors that are actually okay for execution

#[tokio::test]
async fn test_console_log_is_ignored() {
    // TS2580: Cannot find name 'console' should be ignored
    let code = r#"console.log("Hello, World!");"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        result.success,
        "console.log should be allowed (TS2580 should be filtered), got: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.is_empty(),
        "Should have no diagnostics after filtering, got: {:?}",
        result.diagnostics
    );
}

#[tokio::test]
async fn test_promise_is_ignored() {
    // TS2585: 'Promise' only refers to a type, but is being used as a value
    // TS2591: Cannot find name 'Promise'
    // Both should be ignored
    let code = r"
const myPromise = new Promise((resolve) => {
    resolve(42);
});
";

    let result = execute(code, None).await.expect("execution should succeed");

    // The test should pass - Promise-related errors should be filtered
    assert!(
        result.success,
        "Promise usage should be allowed (TS2585/TS2591 should be filtered), got: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.is_empty(),
        "All Promise errors should be filtered out, got: {:?}",
        result.diagnostics
    );
}

#[tokio::test]
async fn test_implicit_any_is_ignored() {
    // TS7006: Parameter implicitly has an 'any' type should be ignored
    let code = r#"
function greet(name) {
    return "Hello, " + name;
}
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        result.success,
        "Implicit any parameters should be allowed (TS7006 should be filtered), got: {:?}",
        result.diagnostics
    );
}

#[tokio::test]
async fn test_dynamic_object_access_is_ignored() {
    // TS7053: Element implicitly has an 'any' type should be ignored
    let code = r#"const obj: Record<string, any> = { key: "value" };
const key = "key";
const value = obj[key];
export default value;"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        result.success,
        "Dynamic object access should be allowed (TS7053 should be filtered), got: diagnostics={:?}, runtime_error={:?}",
        result.diagnostics, result.runtime_error
    );
}

#[tokio::test]
async fn test_relevant_errors_not_filtered() {
    // TS2322: Type error should NOT be filtered
    let code = r#"
const x: number = "string";
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        !result.success,
        "Type mismatch should fail (TS2322 should NOT be filtered)"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "Should have type error diagnostics"
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == Some(2322)),
        "Should include TS2322 error, got: {:?}",
        result.diagnostics
    );
}

#[tokio::test]
async fn test_mixed_errors_only_relevant_shown() {
    // This should have both filtered (console) and unfiltered (type error) diagnostics
    let code = r#"
console.log("This uses console");
const x: number = "string";
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(!result.success, "Should fail due to type error");

    // Should have diagnostics but console error should be filtered out
    assert!(!result.diagnostics.is_empty(), "Should have diagnostics");

    // Should NOT include console error (TS2580)
    assert!(
        !result.diagnostics.iter().any(|d| d.code == Some(2580)),
        "Should not include TS2580 (console) error, got: {:?}",
        result.diagnostics
    );

    // Should include type error (TS2322)
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == Some(2322) || d.message.contains("not assignable")),
        "Should include type mismatch error, got: {:?}",
        result.diagnostics
    );
}
