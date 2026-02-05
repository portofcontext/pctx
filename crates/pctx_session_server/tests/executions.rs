mod utils;

use crate::utils::{callback_tools, connect_websocket, create_test_server_with_session};
use pctx_code_mode::CodeMode;
use pctx_code_mode::model::CallbackConfig;
use pctx_session_server::{CODE_MODE_SESSION_HEADER, PctxSessionBackend, model::WsJsonRpcMessage};
use serde_json::json;
use serial_test::serial;
use similar_asserts::assert_serde_eq;
use uuid::Uuid;

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

    // Should show the actual code context with the error
    assert!(
        stderr.contains("bloop x = 12;"),
        "Should show the line with the error: {stderr}"
    );
}

#[test_log::test(tokio::test)]
#[serial]
async fn test_exec_callbacks() {
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
    let code = "
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

    // Send execute_code request via WebSocket
    ws.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "test-4",
        "method": "execute_code",
        "params": {
            "code": code
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

    // LLM code with type error - this will have namespaces prepended
    // The error is on line 3 of the original code
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

    // Should show exact location: Line 3, Column 45
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

#[test_log::test(tokio::test)]
#[serial]
/// WHY THIS IS FAILING:
/// This is the direct consequence of the sequential worker pool we built. Look at the worker loop in worker_pool.rs:41-47:
///
/// while let Some(req) = rx.recv().await {
///     let result = crate::execute_inner(&req.code, req.options).await;  // ← blocks until complete
///     let _ = req.response_tx.send(result);
/// }
/// Here's the timeline of your test:
///
/// Time	Event
/// t=0s	ws1 sends execute_code → worker picks it up
/// t≈0s	Execution 1 hits Tools.sleep() callback, sends request to ws1, awaits response
/// t=1s	ws2 sends execute_code → sits in channel queue (worker is awaiting exec 1's callback)
/// t≈3s	Test handler responds to ws1 callback, execution 1 completes
/// t≈3s	Worker picks up execution 2 from queue, starts it
/// t≈3s	Execution 2 hits callback, sends request to ws2, awaits response
/// t≈6s	Test handler responds to ws2 callback, execution 2 completes
/// Total: ~7s	(3s + 1s delay + 3s) — fully sequential
/// The worker processes one request at a time. While execution 1 is suspended awaiting its callback response, the worker thread is idle but still .await-ing execute_inner — it never loops back to rx.recv() to pick up execution 2.
///
/// This is exactly the tradeoff we made: we removed spawn_local because interleaving two live JsRuntime instances on the same thread caused the V8 crash. Sequential processing was the fix, but it means executions can never overlap — even when one is just waiting on I/O.
///
/// The fundamental constraint: true concurrency requires either multiple worker threads (which causes V8 platform races) or spawn_local (which causes interleaved JsRuntime crashes). Do you want to explore solutions, or should we adjust the test expectations?
async fn test_concurrent_executions_with_callback() {
    // Create two sessions on the same server
    let (session_id1, server, state) = create_test_server_with_session().await;
    let session_id2 = Uuid::new_v4();
    state
        .backend
        .insert(session_id2, CodeMode::default())
        .await
        .expect("Failed adding second test session");

    // Define a sleep tool
    let sleep_tools = vec![CallbackConfig {
        name: "sleep".into(),
        namespace: "tools".into(),
        description: Some("Sleep for the given number of seconds".into()),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "seconds": {
                    "type": "number",
                    "description": "Number of seconds to sleep"
                }
            },
            "required": ["seconds"]
        })),
        output_schema: Some(json!({
            "type": "string",
            "description": "Result message"
        })),
    }];

    // Register tool on both sessions
    let tools_payload = json!({ "tools": sleep_tools });
    for &session_id in &[session_id1, session_id2] {
        let res = server
            .post("/register/tools")
            .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
            .json(&tools_payload)
            .await;
        res.assert_status_ok();
    }

    // Connect WebSockets for both sessions
    let mut ws1 = connect_websocket(&server, session_id1)
        .await
        .into_websocket()
        .await;
    let mut ws2 = connect_websocket(&server, session_id2)
        .await
        .into_websocket()
        .await;

    let sleep_for: u64 = 3;
    let second_exec_delay: u64 = 1;
    let code = format!(
        r#"
        async function run() {{
            const result = await Tools.sleep({{ seconds: {} }});
            return {{ result }};
        }}
    "#,
        sleep_for
    );

    let start = tokio::time::Instant::now();

    // Send execute_code on ws1 immediately
    ws1.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "exec-1",
        "method": "execute_code",
        "params": { "code": &code }
    }))
    .await;

    // Delay then send execute_code on ws2
    tokio::time::sleep(std::time::Duration::from_secs(second_exec_delay)).await;
    ws2.send_json(&json!({
        "jsonrpc": "2.0",
        "id": "exec-2",
        "method": "execute_code",
        "params": { "code": &code }
    }))
    .await;

    // Handle both callback sequences concurrently
    let (response1, response2) = tokio::join!(
        async {
            let msg: WsJsonRpcMessage = ws1.receive_json().await;
            let (_tool_req, req_id) = msg.into_request().unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(sleep_for)).await;
            ws1.send_json(&json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "output": format!("slept {}s", sleep_for)
                }
            }))
            .await;
            let response: serde_json::Value = ws1.receive_json().await;
            response
        },
        async {
            let msg: WsJsonRpcMessage = ws2.receive_json().await;
            let (_tool_req, req_id) = msg.into_request().unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(sleep_for)).await;
            ws2.send_json(&json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "output": format!("slept {}s", sleep_for)
                }
            }))
            .await;
            let response: serde_json::Value = ws2.receive_json().await;
            response
        }
    );

    let elapsed = start.elapsed();

    // Both executions should succeed
    assert_eq!(
        response1["result"]["success"], true,
        "First execution should succeed: {response1:?}",
    );
    assert_eq!(
        response2["result"]["success"], true,
        "Second execution should succeed: {response2:?}",
    );

    // Verify output
    assert_eq!(
        response1["result"]["output"]["result"],
        format!("slept {sleep_for}s")
    );
    assert_eq!(
        response2["result"]["output"]["result"],
        format!("slept {sleep_for}s")
    );

    // Verify timing proves concurrency:
    // If sequential, total time would be ~(sleep_for * 2 + second_exec_delay) = 7s
    // If concurrent, total time should be ~(sleep_for + second_exec_delay) = 4s
    let max_sequential = sleep_for * 2;
    assert!(
        elapsed.as_secs() < max_sequential,
        "Executions appear sequential ({:.1}s >= {}s). Expected concurrent completion in ~{}s.",
        elapsed.as_secs_f64(),
        max_sequential,
        sleep_for + second_exec_delay,
    );
}
