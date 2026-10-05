//! Sandboxed code must not be able to load modules: no host files, no remote code.

use std::io::Write;

use serde_json::Value;

use super::serial;
use crate::{ExecuteOptions, execute};

const SECRET: &str = "pctx-sandbox-secret-3f9a1c";

/// Attempts a dynamic `import()` of `specifier`, built at runtime so the
/// type checker cannot reject it statically.
async fn try_import(specifier: &str, json: bool) -> Value {
    let options = if json {
        r#"{ with: { type: "json" } }"#
    } else {
        "undefined"
    };
    let code = format!(
        r#"
async function test() {{
    const dynamicImport = (0, eval)("(s, o) => import(s, o)");
    try {{
        const mod = await dynamicImport({specifier:?}, {options});
        return {{ loaded: true, value: JSON.stringify(mod) }};
    }} catch (e) {{
        return {{ loaded: false, error: String(e) }};
    }}
}}
export default await test();
"#
    );

    let result = execute(&code, ExecuteOptions::new())
        .await
        .expect("execution should not error internally");
    assert!(
        result.success,
        "execution itself should succeed: {:?}",
        result.runtime_error
    );
    let output = result.output.expect("should have output");
    assert!(
        !output.to_string().contains(SECRET),
        "{specifier}: secret leaked into output: {output}"
    );
    assert!(
        !result.stdout.contains(SECRET) && !result.stderr.contains(SECRET),
        "{specifier}: secret leaked into stdout/stderr"
    );
    output
}

fn assert_rejected(specifier: &str, output: &Value) {
    assert_eq!(
        output["loaded"],
        Value::Bool(false),
        "{specifier} should not be importable, got: {output}"
    );
}

#[serial]
#[tokio::test]
async fn test_cannot_import_host_json_file() {
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, r#"{{ "token": "{SECRET}" }}"#).unwrap();
    let specifier = format!("file://{}", file.path().display());

    let output = try_import(&specifier, true).await;
    assert_rejected(&specifier, &output);
}

#[serial]
#[tokio::test]
async fn test_cannot_import_host_js_file() {
    let mut file = tempfile::Builder::new().suffix(".js").tempfile().unwrap();
    write!(file, r#"export const token = "{SECRET}";"#).unwrap();
    let specifier = format!("file://{}", file.path().display());

    let output = try_import(&specifier, false).await;
    assert_rejected(&specifier, &output);
}

#[serial]
#[tokio::test]
async fn test_host_file_existence_is_not_observable() {
    let file = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
    let existing = format!("file://{}", file.path().display());
    let missing = format!("file://{}.missing", file.path().display());

    let existing_out = try_import(&existing, false).await;
    let missing_out = try_import(&missing, false).await;
    assert_rejected(&existing, &existing_out);
    assert_rejected(&missing, &missing_out);
    assert_eq!(
        existing_out["error"]
            .as_str()
            .map(|e| e.replace(&existing, "<spec>")),
        missing_out["error"]
            .as_str()
            .map(|e| e.replace(&missing, "<spec>")),
        "errors for existing and missing files should be indistinguishable"
    );
}

#[serial]
#[tokio::test]
async fn test_cannot_import_remote_or_inline_modules() {
    for specifier in [
        "https://example.com/mod.js",
        "http://127.0.0.1:1/mod.js",
        "data:text/javascript,export default 42",
        "node:fs",
        "npm:left-pad",
        "jsr:@std/path",
    ] {
        let output = try_import(specifier, false).await;
        assert_rejected(specifier, &output);
    }
}
