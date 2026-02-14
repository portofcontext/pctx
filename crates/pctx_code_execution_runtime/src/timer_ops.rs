//! Deno ops for timer functionality (setTimeout, setInterval)
//!
//! Provides a simple async sleep op that JavaScript uses to implement timers.

use deno_core::op2;
use std::time::Duration;

/// Sleep for the specified number of milliseconds
///
/// This async op is used by the JavaScript layer to implement setTimeout and setInterval.
/// It simply waits for the specified duration before resolving.
#[op2(async)]
pub(crate) async fn op_sleep(#[bigint] delay_ms: u64) {
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}
