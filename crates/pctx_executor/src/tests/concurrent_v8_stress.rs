//! Stress test for concurrent V8 execution via the worker pool.
//!
//! Multiple tokio tasks call `execute()` concurrently. The worker pool
//! dispatches all work to a single dedicated thread where executions
//! interleave cooperatively — no mutex, no cross-thread V8 races.

use crate::{ExecuteOptions, execute};

#[tokio::test]
async fn test_concurrent_execute_stress() {
    let mut handles = Vec::new();

    for i in 0..4 {
        let handle = tokio::spawn(async move {
            for j in 0..3 {
                let code =
                    format!("const x{i}_{j}: number = {i} + {j}; export default x{i}_{j};");
                let result = execute(&code, ExecuteOptions::new()).await.unwrap();
                assert!(
                    result.success,
                    "iteration {i}_{j} failed: {:?}",
                    result.diagnostics
                );
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }
}
