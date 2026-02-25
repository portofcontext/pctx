mod utils;

use crate::utils::{callback_tools, connect_websocket, create_test_server_with_session};
use pctx_code_mode::model::CallbackConfig;
use pctx_session_server::{CODE_MODE_SESSION_HEADER, model::WsJsonRpcMessage};
use rstest::rstest;
use serde_json::json;
use serial_test::serial;
use similar_asserts::assert_serde_eq;

#[tokio::test]
#[serial]
async fn test_exec_code_only() {
    let (session_id, server, _) = create_test_server_with_session().await;
    let mut ws = connect_websocket(&server, session_id)
        .await
        .into_websocket()
        .await;

    // Send execute_code request via WebSocket
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "test-1",
        "method": "execute_code",
        "params": {
            "code": "async function run() { return 1 + 1; }"
        }
    }))
    .await;

    // Receive response
    let response: serde_json::Value = ws.receive_json().await;

    assert_serde_eq!(
        response,
        json!({
            "jsonrpc": "2.0",
            "id": "test-1",
            "result": {
                "success": true,
                "stdout": "",
                "stderr": "",
                "output": 2
            }
        })
    );
}

#[tokio::test]
#[serial]
async fn test_exec_code_console_output() {
    let (session_id, server, _) = create_test_server_with_session().await;
    let mut ws = connect_websocket(&server, session_id)
        .await
        .into_websocket()
        .await;

    let code = r#"
        async function run() {
            console.log("Test log");
            console.error("Test error");
            return "done";
        }
    "#;

    // Send execute_code request via WebSocket
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "test-2",
        "method": "execute_code",
        "params": {
            "code": code
        }
    }))
    .await;

    // Receive response
    let response: serde_json::Value = ws.receive_json().await;

    assert_serde_eq!(
        response,
        json!({
            "jsonrpc": "2.0",
            "id": "test-2",
            "result": {
                "success": true,
                "stdout": "Test log",
                "stderr": "Test error",
                "output": "done"
            }
        })
    );
}

#[tokio::test]
#[serial]
async fn test_exec_code_syntax_err() {
    let (session_id, server, _) = create_test_server_with_session().await;
    let mut ws = connect_websocket(&server, session_id)
        .await
        .into_websocket()
        .await;

    let invalid_code = "
        async function run() {
            bloop x = 12;
            return x;
        }
    ";

    // Send execute_code request via WebSocket
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "test-3",
        "method": "execute_code",
        "params": {
            "code": invalid_code
        }
    }))
    .await;

    // Receive response
    let response: serde_json::Value = ws.receive_json().await;

    assert_eq!(response["result"]["success"], false);
    let stderr = response["result"]["stderr"].as_str().unwrap();

    // Should show line 3 where the error is
    assert!(
        stderr.contains("3:19"),
        "Should show exact error location (line 3, col 19): {stderr}"
    );

    // Should show the error message
    assert!(
        stderr.contains("Expected"),
        "Should show the error message: {stderr}"
    );
}

const CODE_OVERLOADED_SYNTAX: &str = "
    async function run() {
        let value = await invoke({ name:\"test_math__add\", arguments: {a: 8, b: 2}});
        console.log(`after add: ${value}`);
        value = await invoke({ name:\"test_math__subtract\", arguments: {a: value, b: 5}});
        console.log(`after subtract: ${value}`);
        value = await invoke({ name:\"test_math__multiply\", arguments: {a: value, b: 10}});
        console.log(`after multiply: ${value}`);
        value = await invoke({ name:\"test_math__divide\", arguments: {a: value, b: 2}});
        console.log(`after divide: ${value}`);
        return value;
    }";

const CODE_NAMESPACED_SYNTAX: &str = "
    async function run() {
        let value = await TestMath.add({a: 8, b: 2});
        console.log(`after add: ${value}`);
        value = await TestMath.subtract({a: value, b: 5});
        console.log(`after subtract: ${value}`);
        value = await TestMath.multiply({a: value, b: 10});
        console.log(`after multiply: ${value}`);
        value = await TestMath.divide({a: value, b: 2});
        console.log(`after divide: ${value}`);
        return value;
    }";

#[rstest]
#[case::sidecar(CODE_OVERLOADED_SYNTAX, "sidecar")]
#[case::catalog(CODE_NAMESPACED_SYNTAX, "catalog")]
#[case::filesystem(CODE_NAMESPACED_SYNTAX, "filesystem")]
#[serial]
#[tokio::test]
async fn test_exec_callbacks(#[case] code: &str, #[case] style: &str) {
    let (session_id, server, _) = create_test_server_with_session().await;

    // register tools
    let callbacks = callback_tools();
    let test_tools: Vec<CallbackConfig> = callback_tools().into_iter().map(|(c, _)| c).collect();
    let register_res = server
        .post("/register/tools")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({
            "tools": test_tools,
        }))
        .await;
    register_res.assert_status_ok();

    // kick off execution script that uses all of the tools
    let mut ws = connect_websocket(&server, session_id)
        .await
        .into_websocket()
        .await;

    // Send execute_code request via WebSocket
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "test-4",
        "method": "execute_code",
        "params": {
            "code": code,
            "style": style
        }
    }))
    .await;

    // Confirm websocket handler sequence
    let msg: WsJsonRpcMessage = ws.receive_json().await;
    let (add_msg, req_id) = msg.into_request().unwrap();
    assert_serde_eq!(
        json!(add_msg),
        json!({
            "method": "execute_tool",
            "params": {
                "namespace": "test_math",
                "name": "add",
                "args": {
                    "a": 8,
                    "b": 2,
                }
            }
        })
    );
    let add_output = callbacks[0].1(Some(json!({
        "a": 8,
        "b": 2,
    })))
    .await
    .unwrap();
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "output": add_output
        }
    }))
    .await;

    let msg: WsJsonRpcMessage = ws.receive_json().await;
    let (sub_msg, req_id) = msg.into_request().unwrap();
    assert_serde_eq!(
        json!(sub_msg),
        json!({
            "method": "execute_tool",
            "params": {
                "namespace": "test_math",
                "name": "subtract",
                "args": {
                    "a": 10,
                    "b": 5,
                }
            }
        })
    );
    let sub_output = callbacks[1].1(Some(json!({
        "a": 10,
        "b": 5})))
    .await
    .unwrap();
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "output": sub_output
        }
    }))
    .await;

    let msg: WsJsonRpcMessage = ws.receive_json().await;
    let (mult_msg, req_id) = msg.into_request().unwrap();
    assert_serde_eq!(
        json!(mult_msg),
        json!({
            "method": "execute_tool",
            "params": {
                "namespace": "test_math",
                "name": "multiply",
                "args": {
                    "a": 5,
                    "b": 10,
                }
            }
        })
    );
    let mult_output = callbacks[2].1(Some(json!({
        "a": 5,
        "b": 10,
    })))
    .await
    .unwrap();
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "output": mult_output
        }
    }))
    .await;

    let msg: WsJsonRpcMessage = ws.receive_json().await;
    let (div_msg, req_id) = msg.into_request().unwrap();
    assert_serde_eq!(
        json!(div_msg),
        json!({
            "method": "execute_tool",
            "params": {
                "namespace": "test_math",
                "name": "divide",
                "args": {
                    "a": 50,
                    "b": 2,
                }
            }
        })
    );
    let div_output = callbacks[3].1(Some(json!({
        "a": 50,
        "b": 2,
    })))
    .await
    .unwrap();
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "output": div_output
        }
    }))
    .await;

    // Receive the execute_code response
    let response: serde_json::Value = ws.receive_json().await;
    assert_serde_eq!(
        response,
        json!({
            "jsonrpc": "2.0",
            "id": "test-4",
            "result": {
                "success": true,
                "stdout": "after add: 10\nafter subtract: 5\nafter multiply: 50\nafter divide: 25",
                "stderr": "",
                "output": 25
            }
        })
    );
}

#[tokio::test]
#[serial]
async fn test_exec_type_error_with_rich_diagnostics() {
    let (session_id, server, _) = create_test_server_with_session().await;

    // Register tools to generate namespaces
    let test_tools: Vec<CallbackConfig> = callback_tools().into_iter().map(|(c, _)| c).collect();
    let register_res = server
        .post("/register/tools")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({
            "tools": test_tools,
        }))
        .await;
    register_res.assert_status_ok();

    // LLM code with type error
    // The error is on line 3 of user code
    let code = r#"
        async function run() {
            let value = await TestMath.add({a: "wrong", b: 2});  // Type error: 'a' should be number
            return value;
        }
    "#;

    let mut ws = connect_websocket(&server, session_id)
        .await
        .into_websocket()
        .await;

    // Send execute_code request via WebSocket
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "test-type-error",
        "method": "execute_code",
        "params": {
            "code": code
        }
    }))
    .await;

    // Receive response
    let response: serde_json::Value = ws.receive_json().await;

    assert_eq!(response["result"]["success"], false);

    // Verify the diagnostic points to the exact error location and has all the information
    // Error is at line 3 (where "wrong" is passed), column 45 (the "wrong" string literal)
    let stderr = response["result"]["stderr"].as_str().unwrap();

    // Should show exact location: Line 15, Column 45
    assert!(
        stderr.contains("Line 3"),
        "Should show line 3 where error occurs: {stderr}"
    );
    assert!(
        stderr.contains("Column 45"),
        "Should show column 45 where 'wrong' starts: {stderr}"
    );

    // Should show TypeScript error code TS2322 (type not assignable)
    assert!(
        stderr.contains("TS2322"),
        "Should show TS2322 error code: {stderr}"
    );

    // Should show the exact type error message
    assert!(
        stderr.contains("Type 'string' is not assignable to type 'number'"),
        "Should show exact type mismatch: {stderr}"
    );
}

#[tokio::test]
#[serial]
async fn test_explore_virtual_fs_with_bash() {
    let (session_id, server, _) = create_test_server_with_session().await;

    // Register tools to populate virtual filesystem
    let test_tools: Vec<CallbackConfig> = callback_tools().into_iter().map(|(c, _)| c).collect();
    let register_res = server
        .post("/register/tools")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({
            "tools": test_tools,
        }))
        .await;
    register_res.assert_status_ok();

    // Test 1: List files in SDK directory (cwd is /sdk/)
    let response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "ls" }))
        .await
        .json();

    assert_eq!(response["success"], true);
    let stdout = response["stdout"].as_str().unwrap();

    // Should list README.md and TestMath namespace folder (no system dirs!)
    assert!(
        stdout.contains("README.md"),
        "Should have README.md: {stdout}"
    );
    assert!(
        stdout.contains("TestMath"),
        "Should have TestMath namespace folder: {stdout}"
    );
    assert!(
        !stdout.contains("bin"),
        "Should NOT have system bin dir: {stdout}"
    );
    assert!(
        !stdout.contains("proc"),
        "Should NOT have system proc dir: {stdout}"
    );

    // Test 2: Read README.md
    let response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "cat README.md" }))
        .await
        .json();

    assert_eq!(response["success"], true);
    let readme = response["stdout"].as_str().unwrap();

    // README should contain function listings
    assert!(
        readme.contains("# TypeScript SDK"),
        "Should have header: {readme}"
    );
    assert!(
        readme.contains("## TestMath"),
        "Should have TestMath namespace: {readme}"
    );
    assert!(readme.contains("add"), "Should list add function: {readme}");
    assert!(
        readme.contains("subtract"),
        "Should list subtract function: {readme}"
    );
    assert!(
        readme.contains("multiply"),
        "Should list multiply function: {readme}"
    );
    assert!(
        readme.contains("divide"),
        "Should list divide function: {readme}"
    );

    // Test 3: Grep for specific function
    let response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "grep 'add' README.md" }))
        .await
        .json();

    assert_eq!(response["success"], true);
    let grep_result = response["stdout"].as_str().unwrap();

    assert!(
        grep_result.contains("add"),
        "Should find add function: {grep_result}"
    );

    // Test 4: Read individual tool TypeScript definition
    let response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "cat TestMath/add.d.ts" }))
        .await
        .json();

    assert_eq!(response["success"], true);
    let types = response["stdout"].as_str().unwrap();

    // Should contain function signature with types
    assert!(
        types.contains("function add"),
        "Should have add function: {types}"
    );
    assert!(
        types.contains("a: number"),
        "Should have typed parameters: {types}"
    );

    // Test 5: List files in TestMath namespace directory
    let response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "ls TestMath/" }))
        .await
        .json();

    assert_eq!(response["success"], true);
    let tools_list = response["stdout"].as_str().unwrap();

    // Should list individual tool files
    assert!(
        tools_list.contains("add.d.ts"),
        "Should have add.d.ts: {tools_list}"
    );
    assert!(
        tools_list.contains("subtract.d.ts"),
        "Should have subtract.d.ts: {tools_list}"
    );
    assert!(
        tools_list.contains("multiply.d.ts"),
        "Should have multiply.d.ts: {tools_list}"
    );
    assert!(
        tools_list.contains("divide.d.ts"),
        "Should have divide.d.ts: {tools_list}"
    );
}

#[test_log::test(tokio::test)]
#[serial]
async fn test_bash_exploration_then_typescript_execution() {
    let (session_id, server, _) = create_test_server_with_session().await;

    // Register tools
    let callbacks = callback_tools();
    let test_tools: Vec<CallbackConfig> = callbacks.iter().map(|(c, _)| c.clone()).collect();
    let register_res = server
        .post("/register/tools")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({
            "tools": test_tools,
        }))
        .await;
    register_res.assert_status_ok();

    // Step 1: LLM explores the filesystem to discover available functions using bash
    // List files (cwd is /sdk/)
    let ls_response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "ls" }))
        .await
        .json();

    assert_eq!(ls_response["success"], true);
    let files_found = ls_response["stdout"].as_str().unwrap();
    assert!(files_found.contains("README.md"));
    assert!(files_found.contains("TestMath"));
    assert!(!files_found.contains("bin"), "Should NOT see system dirs");

    // Read README
    let readme_response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "cat README.md" }))
        .await
        .json();

    assert_eq!(readme_response["success"], true);
    let readme = readme_response["stdout"].as_str().unwrap();
    assert!(readme.contains("## TestMath"));

    // Search for math functions
    let grep_response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "grep -E '(add|multiply)' README.md" }))
        .await
        .json();

    assert_eq!(grep_response["success"], true);
    let math_functions = grep_response["stdout"].as_str().unwrap();
    assert!(math_functions.contains("add"));
    assert!(math_functions.contains("multiply"));

    // Read individual tool type definition
    let types_response: serde_json::Value = server
        .post("/code-mode/execute-bash")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({ "command": "cat TestMath/add.d.ts" }))
        .await
        .json();

    assert_eq!(types_response["success"], true);
    let types = types_response["stdout"].as_str().unwrap();
    assert!(types.contains("function add"));

    // Step 2: Now use the discovered information to execute TypeScript code
    let mut ws = connect_websocket(&server, session_id)
        .await
        .into_websocket()
        .await;
    let execution_code = r#"
        async function run() {
            // Based on bash exploration, we know:
            // - TestMath namespace exists
            // - add(a: number, b: number) function is available
            // - multiply(a: number, b: number) function is available

            const sum = await TestMath.add({a: 10, b: 5});
            console.log(`Sum: ${sum}`);

            const product = await TestMath.multiply({a: sum, b: 3});
            console.log(`Product: ${product}`);

            return { sum, product };
        }
    "#;

    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "execute",
        "method": "execute_code",
        "params": { "code": execution_code }
    }))
    .await;

    // Handle the add callback
    let msg: WsJsonRpcMessage = ws.receive_json().await;
    let (add_msg, req_id) = msg.into_request().unwrap();
    assert_serde_eq!(
        json!(add_msg),
        json!({
            "method": "execute_tool",
            "params": {
                "namespace": "test_math",
                "name": "add",
                "args": {
                    "a": 10,
                    "b": 5,
                }
            }
        })
    );
    let add_output = callbacks[0].1(Some(json!({"a": 10, "b": 5})))
        .await
        .unwrap();
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": { "output": add_output }
    }))
    .await;

    // Handle the multiply callback
    let msg: WsJsonRpcMessage = ws.receive_json().await;
    let (mult_msg, req_id) = msg.into_request().unwrap();
    assert_serde_eq!(
        json!(mult_msg),
        json!({
            "method": "execute_tool",
            "params": {
                "namespace": "test_math",
                "name": "multiply",
                "args": {
                    "a": 15,
                    "b": 3,
                }
            }
        })
    );
    let mult_output = callbacks[2].1(Some(json!({"a": 15, "b": 3})))
        .await
        .unwrap();
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": { "output": mult_output }
    }))
    .await;

    // Receive final result
    let response: serde_json::Value = ws.receive_json().await;
    assert_serde_eq!(
        response,
        json!({
            "jsonrpc": "2.0",
            "id": "execute",
            "result": {
                "success": true,
                "stdout": "Sum: 15\nProduct: 45",
                "stderr": "",
                "output": {
                    "sum": 15,
                    "product": 45
                }
            }
        })
    );
}
