use pctx_code_mode::model::CallbackConfig;
use pctx_session_server::CODE_MODE_SESSION_HEADER;
use serde_json::json;

use crate::utils::{
    callback_tools, create_session, create_test_server, create_test_server_with_session,
};

mod utils;

#[tokio::test]
async fn test_health_endpoint() {
    let (server, _) = create_test_server();

    let res = server.get("/health").await;

    res.assert_status_ok();
    res.assert_json(&json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }));
}

#[tokio::test]
async fn test_register_tools() {
    let (session_id, server, _state) = create_test_server_with_session().await;
    let test_tools: Vec<CallbackConfig> = callback_tools().into_iter().map(|(c, _)| c).collect();

    let res = server
        .post("/register/tools")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({
            "tools": test_tools,
        }))
        .await;

    res.assert_status_ok();
    res.assert_json(&json!({"registered": test_tools.len(), "failed": []}));

    // List functions & get details
    let list_res = server
        .post("/code-mode/functions/list")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .await;
    list_res.assert_status_ok();
    list_res.assert_json_contains(&json!({
        "functions": [
            {
                "namespace": "TestMath",
                "name": "add",
                "description": "Add two numbers & return result"
            },
            {
                "namespace": "TestMath",
                "name": "subtract",
                "description": "Subtract two numbers & return result"
            },
            {
                "namespace": "TestMath",
                "name": "multiply",
                "description": "Multiply two numbers & return result"
            },
            {
                "namespace": "TestMath",
                "name": "divide",
                "description": "Divide two numbers & return result"
            }
        ],
    }));

    let details_res = server
        .post("/code-mode/functions/details")
        .add_header(CODE_MODE_SESSION_HEADER, session_id.to_string())
        .json(&json!({"functions": ["TestMath.add", "TestMath.subtract", "TestMath.multiply", "TestMath.divide"]}))
        .await;
    details_res.assert_status_ok();
    details_res.assert_json_contains(&json!({
        "functions": [
            {
                "namespace": "TestMath",
                "name": "add",
                "description": "Add two numbers & return result",
                "input_type": "AddInput",
                "output_type": "number",
            },
            {
                "namespace": "TestMath",
                "name": "subtract",
                "description": "Subtract two numbers & return result",
                "input_type": "SubtractInput",
                "output_type": "number",
            },
            {
                "namespace": "TestMath",
                "name": "multiply",
                "description": "Multiply two numbers & return result",
                "input_type": "MultiplyInput",
                "output_type": "number",
            },
            {
                "namespace": "TestMath",
                "name": "divide",
                "description": "Divide two numbers & return result",
                "input_type": "DivideInput",
                "output_type": "number",
            }
        ],
    }));
}

#[tokio::test]
async fn test_register_tools_not_shared() {
    let (server, _) = create_test_server();
    let session_1 = create_session(&server).await;
    let test_tools: Vec<CallbackConfig> = callback_tools().into_iter().map(|(c, _)| c).collect();
    // register tools with session 1
    let res = server
        .post("/register/tools")
        .add_header(CODE_MODE_SESSION_HEADER, session_1.to_string())
        .json(&json!({
            "tools": test_tools,
        }))
        .await;

    res.assert_status_ok();
    res.assert_json(&json!({"registered": test_tools.len(), "failed": []}));

    let session_2 = create_session(&server).await;

    // List functions & get details with session 2 (should be empty)
    let list_res = server
        .post("/code-mode/functions/list")
        .add_header(CODE_MODE_SESSION_HEADER, session_2.to_string())
        .await;
    list_res.assert_status_ok();
    list_res.assert_json_contains(&json!({"functions": []}));

    let details_res = server
        .post("/code-mode/functions/details")
        .add_header(CODE_MODE_SESSION_HEADER, session_2.to_string())
        .json(&json!({"functions": ["TestMath.add", "TestMath.subtract", "TestMath.multiply", "TestMath.divide"]}))
        .await;
    details_res.assert_status_ok();
    details_res.assert_json_contains(&json!({"functions": []}));
}
