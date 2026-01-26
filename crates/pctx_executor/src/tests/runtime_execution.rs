use super::serial;
use crate::{ExecuteOptions, execute};

#[tokio::test]
#[serial]
async fn test_execute_simple_code() {
    let code = r"
const x = 1 + 1;
export default x;
";

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
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

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
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

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
    assert!(!result.success, "Code with syntax error should fail");
    // Syntax errors are caught during execution
    assert!(
        result.runtime_error.is_some() || !result.diagnostics.is_empty(),
        "Should have error information"
    );
}


#[tokio::test]
#[serial]
async fn test_just_bash_ls() {
    let code = r#"
async function test() {
    const bash = new Bash({
        files: {
            "/app/file1.txt": "content1",
            "/app/file2.txt": "content2",
        },
        cwd: "/app",
    });
    const result = await bash.exec("ls");
    return result.stdout.trim();
}

export default await test();
"#;

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
    assert!(
        result.success,
        "just-bash ls should work. diagnostics: {:?}, runtime_error: {:?}, stderr: {}",
        result.diagnostics,
        result.runtime_error,
        result.stderr
    );
    assert!(result.runtime_error.is_none(), "Should have no runtime errors");
    // ls output should contain both files
    let output = result.output.unwrap();
    let output_str = output.as_str().unwrap();
    assert!(output_str.contains("file1.txt"), "ls should list file1.txt");
    assert!(output_str.contains("file2.txt"), "ls should list file2.txt");
}

#[tokio::test]
#[serial]
async fn test_just_bash_head() {
    let code = r#"
async function test() {
    const bash = new Bash({
        files: {
            "/app/lines.txt": "line1\nline2\nline3\nline4\nline5\nline6",
        },
        cwd: "/app",
    });
    const result = await bash.exec("head -n 3 lines.txt");
    return result.stdout;
}

export default await test();
"#;

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
    assert!(
        result.success,
        "just-bash head should work. diagnostics: {:?}, runtime_error: {:?}, stderr: {}",
        result.diagnostics,
        result.runtime_error,
        result.stderr
    );
    assert!(result.runtime_error.is_none(), "Should have no runtime errors");
    assert_eq!(
        result.output,
        Some(serde_json::json!("line1\nline2\nline3\n")),
        "head should return first 3 lines"
    );
}

#[tokio::test]
#[serial]
async fn test_just_bash_grep() {
    let code = r#"
async function test() {
    const bash = new Bash({
        files: {
            "/app/data.txt": "apple\nbanana\napricot\ncherry\navocado",
        },
        cwd: "/app",
    });
    const result = await bash.exec("grep '^a' data.txt");
    return result.stdout;
}

export default await test();
"#;

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
    assert!(
        result.success,
        "just-bash grep should work. diagnostics: {:?}, runtime_error: {:?}, stderr: {}",
        result.diagnostics,
        result.runtime_error,
        result.stderr
    );
    assert!(result.runtime_error.is_none(), "Should have no runtime errors");
    assert_eq!(
        result.output,
        Some(serde_json::json!("apple\napricot\navocado\n")),
        "grep should match lines starting with 'a'"
    );
}

#[tokio::test]
#[serial]
async fn test_just_bash_find() {
    let code = r##"
async function test() {
    const bash = new Bash({
        files: {
            "/app/src/main.ts": "console.log('hello')",
            "/app/src/util.ts": "export const x = 1",
            "/app/test/main.test.ts": "test('works', () => {})",
            "/app/readme.md": "# Readme",
        },
        cwd: "/app",
    });
    const result = await bash.exec("find . -name '*.ts'");
    return result.stdout;
}

export default await test();
"##;

    let result = execute(code, ExecuteOptions::new())
        .await
        .expect("execution should succeed");
    assert!(
        result.success,
        "just-bash find should work. diagnostics: {:?}, runtime_error: {:?}, stderr: {}",
        result.diagnostics,
        result.runtime_error,
        result.stderr
    );
    assert!(result.runtime_error.is_none(), "Should have no runtime errors");
    let output = result.output.unwrap();
    let output_str = output.as_str().unwrap();
    assert!(output_str.contains("main.ts"), "find should locate main.ts");
    assert!(output_str.contains("util.ts"), "find should locate util.ts");
    assert!(output_str.contains("main.test.ts"), "find should locate main.test.ts");
    assert!(!output_str.contains("readme.md"), "find should not match readme.md");
}
