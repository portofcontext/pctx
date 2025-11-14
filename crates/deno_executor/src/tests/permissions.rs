use super::serial;
use crate::execute;

#[serial]
#[tokio::test]
async fn test_network_blocked_when_no_hosts_allowed() {
    let code = r#"
async function test() {
    try {
        await fetch("https://example.com");
        return { success: true };
    } catch (e) {
        return {
            success: false,
            error: e.message,
            isPermissionError: e.message.includes("Network access") || e.message.includes("not allowed")
        };
    }
}

export default await test();
"#;

    let result = execute(code, None).await.expect("execution should succeed");
    assert!(result.success, "Execution should succeed");

    let output = result.output.expect("Should have output");
    let obj = output.as_object().expect("Should be an object");

    assert_eq!(
        obj.get("success").unwrap(),
        &serde_json::json!(false),
        "Network request should be blocked"
    );
    assert_eq!(
        obj.get("isPermissionError").unwrap(),
        &serde_json::json!(true),
        "Should be a permission error. Got error: {:?}",
        obj.get("error")
    );
}

#[serial]
#[tokio::test]
async fn test_network_allowed_for_specific_host() {
    // Initialize rustls crypto provider for network requests
    super::init_rustls_crypto();

    let code = r#"
async function test() {
    try {
        // This will fail with connection error (no server), but permission is granted
        await fetch("http://localhost:8888/test");
        return { gotPermission: true, connected: true };
    } catch (e) {
        // If it's a network error (not permission), we got permission
        const gotPermission = !e.message.includes("Network access") && !e.message.includes("not allowed");
        return {
            gotPermission,
            connected: false,
            error: e.message
        };
    }
}

export default await test();
"#;

    let allowed_hosts = Some(vec!["localhost:8888".to_string()]);
    let result = execute(code, allowed_hosts)
        .await
        .expect("execution should succeed");
    assert!(result.success, "Execution should succeed");

    let output = result.output.expect("Should have output");
    let obj = output.as_object().expect("Should be an object");

    assert_eq!(
        obj.get("gotPermission").unwrap(),
        &serde_json::json!(true),
        "Should have network permission. Error: {:?}",
        obj.get("error")
    );
}

#[serial]
#[tokio::test]
async fn test_network_blocked_for_different_host() {
    let code = r#"
async function test() {
    try {
        await fetch("http://example.com");
        return { blocked: false };
    } catch (e) {
        const isPermissionError = e.message.includes("Network access") || e.message.includes("not allowed");
        return {
            blocked: isPermissionError,
            error: e.message
        };
    }
}

export default await test();
"#;

    // Allow localhost:3000 but try to access example.com
    let allowed_hosts = Some(vec!["localhost:3000".to_string()]);
    let result = execute(code, allowed_hosts)
        .await
        .expect("execution should succeed");
    assert!(result.success, "Execution should succeed");

    let output = result.output.expect("Should have output");
    let obj = output.as_object().expect("Should be an object");

    assert_eq!(
        obj.get("blocked").unwrap(),
        &serde_json::json!(true),
        "Request to different host should be blocked. Error: {:?}",
        obj.get("error")
    );
}

#[serial]
#[tokio::test]
async fn test_network_allowed_for_multiple_hosts() {
    // Initialize rustls crypto provider for network requests
    super::init_rustls_crypto();

    let code = r#"
async function testHost(host) {
    try {
        await fetch(`http://${host}/test`);
        return { host, gotPermission: true, connected: true };
    } catch (e) {
        const gotPermission = !e.message.includes("Network access") && !e.message.includes("not allowed");
        return { host, gotPermission, connected: false };
    }
}

async function main() {
    const results = await Promise.all([
        testHost("localhost:3000"),
        testHost("localhost:4000"),
        testHost("example.com")
    ]);
    return results;
}

export default await main();
"#;

    let allowed_hosts = Some(vec![
        "localhost:3000".to_string(),
        "localhost:4000".to_string(),
    ]);
    let result = execute(code, allowed_hosts)
        .await
        .expect("execution should succeed");

    if !result.success {
        eprintln!("Execution failed!");
        if let Some(ref err) = result.runtime_error {
            eprintln!("Runtime error: {}", err.message);
        }
        eprintln!("Diagnostics: {:?}", result.diagnostics);
        eprintln!("Stderr: {}", result.stderr);
    }
    assert!(result.success, "Execution should succeed");

    let output = result.output.expect("Should have output");
    let results = output.as_array().expect("Should be an array");

    // localhost:3000 should have permission
    let host3000 = results[0].as_object().unwrap();
    assert_eq!(
        host3000.get("gotPermission").unwrap(),
        &serde_json::json!(true),
        "localhost:3000 should have permission"
    );

    // localhost:4000 should have permission
    let host4000 = results[1].as_object().unwrap();
    assert_eq!(
        host4000.get("gotPermission").unwrap(),
        &serde_json::json!(true),
        "localhost:4000 should have permission"
    );

    // example.com should NOT have permission
    let example = results[2].as_object().unwrap();
    assert_eq!(
        example.get("gotPermission").unwrap(),
        &serde_json::json!(false),
        "example.com should NOT have permission"
    );
}
