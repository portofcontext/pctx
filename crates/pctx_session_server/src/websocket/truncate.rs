//! Bounds `execute_code` responses to fit the WebSocket frame limit.
//! Temporary solution until long term solution is implemented, see plans/large-execution-payloads.md

use pctx_code_mode::model::{ExecuteTypescriptOutput, ExecutionTrace};
use serde_json::json;
use tracing::warn;

/// Response-size ceiling. tungstenite's default `max_frame_size` is 16 MiB and
/// is enforced by the receiver (the client), which can't advertise its cap to
/// us — so we stay under the default with a margin.
pub(super) const MAX_RESPONSE_BYTES: usize = 15 * 1024 * 1024;

/// Serialized wire size of an execution output.
fn serialized_len(output: &ExecuteTypescriptOutput) -> usize {
    serde_json::to_vec(output).map_or(0, |v| v.len())
}

/// Shrink an oversized response to fit [`MAX_RESPONSE_BYTES`]: first drop the
/// trace, then, if still too large, replace the return value with a marker and
/// note on stderr why it's gone so the calling agent can see it.
pub(super) fn bound_response_size(output: &mut ExecuteTypescriptOutput) {
    if serialized_len(output) <= MAX_RESPONSE_BYTES {
        return;
    }

    // The trace clones every tool output, so it's usually the culprit.
    let before = serialized_len(output);
    output.trace = ExecutionTrace::default();
    warn!("execute_code response {before} bytes over {MAX_RESPONSE_BYTES}; dropped trace");

    if serialized_len(output) <= MAX_RESPONSE_BYTES {
        return;
    }

    // The return value / stdout are themselves too large.
    let oversized = serialized_len(output);
    let notice = format!(
        "[truncated: output was {oversized} bytes, over the {MAX_RESPONSE_BYTES}-byte transport limit; return value not delivered]"
    );
    warn!("{notice}");
    output.output = Some(json!({ "__truncated__": true, "reason": notice.clone() }));
    output.stdout = String::new();
    if output.stderr.is_empty() {
        output.stderr = notice;
    } else {
        output.stderr.push('\n');
        output.stderr.push_str(&notice);
    }
}
