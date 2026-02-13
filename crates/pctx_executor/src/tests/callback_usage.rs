use std::sync::Arc;

use pctx_code_execution_runtime::CallbackRegistry;
use serde::Deserialize;
use serde_json::json;

use super::serial;
use crate::{ExecuteOptions, execute};

#[serial]
#[tokio::test]
async fn test_execute_with_callbacks() {
    let registry = CallbackRegistry::default();
    registry
        .add(
            "MyMath.add",
            Arc::new(move |args: Option<serde_json::Value>| {
                Box::pin(async move {
                    #[derive(Deserialize)]
                    struct AddArgs {
                        a: isize,
                        b: isize,
                    }

                    let args = args.ok_or_else(|| "Missing arguments".to_string())?;
                    let parsed: AddArgs =
                        serde_json::from_value(args).map_err(|e| format!("Invalid args: {e}"))?;

                    Ok(json!(parsed.a + parsed.b))
                })
            }),
        )
        .expect("callback registration should succeed");

    let code = r#"
async function test() {
    try {
        const val = await invokeCallback({ id: "MyMath.add", arguments: { a: 12, b: 4 } });
        return { error: false, value: val };
    } catch (e) {
        return { error: true, message: e instanceof Error ? e.message : String(e) };
    }
}

export default await test();
"#;

    let result = execute(code, ExecuteOptions::new().with_callbacks(registry))
        .await
        .expect("execution should succeed");

    assert_eq!(result.output, Some(json!({"error": false, "value": 16})));
    assert!(
        result.success,
        "Code with callbacks should execute successfully"
    );
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
    assert!(result.diagnostics.is_empty(), "Should have no type errors");
}

#[serial]
#[tokio::test]
async fn test_execute_with_async_callbacks() {
    let registry = CallbackRegistry::default();
    registry
        .add(
            "MyAsync.wait",
            Arc::new(move |args: Option<serde_json::Value>| {
                Box::pin(async move {
                    #[derive(Deserialize)]
                    struct WaitArgs {
                        ms: u64,
                    }

                    let args = args.ok_or_else(|| "Missing arguments".to_string())?;
                    let parsed: WaitArgs =
                        serde_json::from_value(args).map_err(|e| format!("Invalid args: {e}"))?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(parsed.ms)).await;

                    Ok(json!(format!("Waited for {}ms", parsed.ms)))
                })
            }),
        )
        .expect("callback registration should succeed");

    let code = r#"
async function test() {
    try {
        const val = await invokeCallback({ id: "MyAsync.wait", arguments: { ms: 50 } });
        return { error: false, value: val };
    } catch (e) {
        return { error: true, message: e instanceof Error ? e.message : String(e) };
    }
}

export default await test();
"#;

    let result = execute(code, ExecuteOptions::new().with_callbacks(registry))
        .await
        .expect("execution should succeed");

    assert_eq!(
        result.output,
        Some(json!({"error": false, "value": "Waited for 50ms"}))
    );
    assert!(
        result.success,
        "Code with callbacks should execute successfully"
    );
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
    assert!(result.diagnostics.is_empty(), "Should have no type errors");
}
