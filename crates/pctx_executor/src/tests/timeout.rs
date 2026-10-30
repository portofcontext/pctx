use super::serial;
use crate::{ExecuteOptions, execute};
use serde_json::json;
use std::time::Duration;

fn one_second() -> ExecuteOptions {
    ExecuteOptions::new().with_timeout(Duration::from_secs(1))
}

#[tokio::test]
#[serial]
async fn a_busy_loop_is_stopped_and_the_next_run_works() {
    let result = execute("while (true) {}", one_second())
        .await
        .expect("executor should report the timeout as a failed run");
    assert!(!result.success);
    assert!(
        result.stderr.contains("exceeded 1s"),
        "stderr: {}",
        result.stderr
    );

    let next = execute("export default 1 + 1;", ExecuteOptions::new())
        .await
        .expect("the next run should not wait on the stopped one");
    assert_eq!(next.output, Some(json!(2)));
}

#[tokio::test]
#[serial]
async fn a_run_waiting_forever_is_stopped() {
    let code = r"
async function test() {
    await new Promise((r) => setTimeout(r, 600_000));
}
export default await test();
";
    let result = execute(code, one_second())
        .await
        .expect("executor should report the timeout as a failed run");
    assert!(!result.success);
    assert!(
        result.stderr.contains("exceeded 1s"),
        "stderr: {}",
        result.stderr
    );
}
