//! Stress test for concurrent V8 isolate usage.
//!
//! Deliberately does NOT use #[serial] — the goal is to reproduce the
//! V8 vector-out-of-bounds crash that occurs when type_check and
//! execute_code create JsRuntimes on different OS threads simultaneously.

use crate::{ExecuteOptions, execute};

#[test]
fn test_concurrent_execute_stress() {
    // Mirror the production pattern: multiple OS threads each with their own
    // single-threaded tokio runtime, calling execute() (type_check + execute_code)
    // concurrently. This creates overlapping V8 isolates without serialization.
    let handles: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async {
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
                })
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
