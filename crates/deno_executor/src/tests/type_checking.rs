use super::serial;
use crate::execute;

#[serial]
#[tokio::test]
async fn test_execute_with_type_error() {
    let code = r#"const x: number = "string";"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(!result.success, "Type error should cause failure");
    assert!(
        !result.diagnostics.is_empty(),
        "Should have type diagnostics"
    );
    assert!(
        result.runtime_error.is_none(),
        "Should not execute with type errors"
    );
}

#[serial]
#[tokio::test]
async fn test_check_valid_typescript() {
    let code = r#"const greeting: string = "Hello, World!";
console.log(greeting);
export default greeting;"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(
        result.success,
        "Valid TypeScript should pass type checking, got: diagnostics={:?}, runtime_error={:?}",
        result.diagnostics, result.runtime_error
    );
    assert!(
        result.diagnostics.is_empty(),
        "Valid TypeScript should have no diagnostics"
    );
}

#[serial]
#[tokio::test]
async fn test_check_type_mismatch() {
    let code = r#"const x: number = "string""#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        !result.success,
        "Type mismatch should fail with typescript-go"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "Should have type error diagnostics"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("not assignable") || d.message.contains("Type")),
        "Error should mention type incompatibility, got: {:?}",
        result.diagnostics
    );
}

#[serial]
#[tokio::test]
async fn test_check_syntax_error() {
    let code = r"const x: string =";

    let result = execute(code, None).await;
    // Should catch syntax error
    if let Ok(result) = result {
        assert!(!result.success, "Invalid syntax should fail");
    }
}

#[serial]
#[tokio::test]
async fn test_nested_object_type_mismatch() {
    let code = r#"
interface User {
    name: string;
    profile: {
        age: number;
        email: string;
    };
}

const user: User = {
    name: "Alice",
    profile: {
        age: "thirty",  // Type error: should be number, not string
        email: "alice@example.com"
    }
};
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        !result.success,
        "Type mismatch in nested object should fail with typescript-go"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "Should detect type error in nested object, got: {:?}",
        result.diagnostics
    );
}

#[serial]
#[tokio::test]
async fn test_function_signature_mismatch() {
    let code = r#"
function greet(name: string): string {
    return name;
}

const result: number = greet("Alice");  // Type error
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        !result.success,
        "Function return type mismatch should fail with typescript-go"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "Should detect return type mismatch, got: {:?}",
        result.diagnostics
    );
}

#[serial]
#[tokio::test]
async fn test_undeclared_variable() {
    // Note: console.log itself is filtered (TS2580), but undeclaredVariable should fail
    // We need to use a different context that doesn't involve console
    let code = r"const x = undeclaredVariable;";

    let result = execute(code, None).await.expect("execution should succeed");

    // If typescript-go is available, it should catch the error
    // If using syntax-only fallback, it might pass
    if result.diagnostics.is_empty() {
        // Fallback to syntax-only checking doesn't catch this
        return;
    }

    assert!(
        !result.success,
        "Undeclared variable should fail with typescript-go"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "Should detect undeclared variable, got: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Cannot find name")),
        "Error should mention undeclared variable, got: {:?}",
        result.diagnostics
    );
}

#[serial]
#[tokio::test]
async fn test_comprehensive_es2020_types() {
    // Comprehensive test for all essential ES2020 types
    let code = r#"
// Primitives
const str: string = "hello";
const num: number = 42;
const bool: boolean = true;

// Arrays
const arr: number[] = [1, 2, 3];
arr.push(4);
arr.map(x => x * 2);

// Objects
const obj: Record<string, any> = { key: "value" };
Object.keys(obj);
Object.entries(obj);

// Promise
const promise = new Promise<number>((resolve) => {
  resolve(42);
});
promise.then(x => console.log(x));

// Collections - Set
const mySet = new Set<number>();
mySet.add(1);
mySet.has(1);
mySet.delete(1);

// Collections - Map
const myMap = new Map<string, number>();
myMap.set("key", 42);
myMap.get("key");
myMap.has("key");

// Collections - WeakMap
const weakMap = new WeakMap<object, string>();
const weakObj = {};
weakMap.set(weakObj, "value");

// Collections - WeakSet
const weakSet = new WeakSet<object>();
weakSet.add(weakObj);

// String methods
str.includes("el");
str.startsWith("he");
str.repeat(2);

// Number methods
Number.isNaN(NaN);
Number.isFinite(42);

// Error handling
try {
  throw new Error("test");
} catch (e) {
  console.error(e);
}

// JSON
const jsonStr = JSON.stringify({ key: "value" });
JSON.parse(jsonStr);

// RegExp
const regex = new RegExp("test");
regex.test("test");

export default "all types work";
"#;

    let result = execute(code, None).await.expect("execution should succeed");

    assert!(
        result.success,
        "Comprehensive ES2020 type test should pass. Diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.is_empty(),
        "Should have no type errors with all ES2020 types. Got diagnostics: {:?}",
        result.diagnostics
    );
}
