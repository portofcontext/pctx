# Tool call concurrency controls

## Problem
Tool calls used to be dispatched **one at a time**: the Python client awaited each tool inside its WebSocket read loop, so it never read the next request until the current tool finished. Code fanning out with `Promise.all` took the *sum* of its calls instead of the slowest one. Everything above the client was already concurrent — the Deno op is `#[op2(async)]` ([invoke_ops.rs:13](../crates/pctx_code_execution_runtime/src/invoke_ops.rs#L13)), the registry holds no lock across its await ([registry.rs:241](../crates/pctx_registry/src/registry.rs#L241)), and the server gives each call its own request id and response channel ([ws_manager.rs:146](../crates/pctx_session_server/src/state/ws_manager.rs#L146)). The requests left the server together and queued at the client.

## Current solution (shipped)
[`_websocket_client.py`](../pctx-py/src/pctx_client/_websocket_client.py):
1. Each `ExecuteToolRequest` runs in its own `asyncio.Task`, so the read loop stays free. Tasks are held in a set (asyncio keeps only weak references) and cancelled as a group on disconnect.
2. Sync tools run via `asyncio.to_thread` — calling one inline blocked the event loop for its whole duration, stalling every other in-flight call behind it.

Measured, 4 × 2s tool under `Promise.all`: **8.12s → 2.11s**, all four starting at t=0.14. Covered by `test_concurrent_tool_calls_run_in_parallel` and `test_concurrent_sync_tool_calls_run_in_parallel` ([test_integration.py](../pctx-py/tests/test_integration.py)), which assert on observed overlap rather than wall time so they fail on serialization, not on a slow machine.

**Dispatch is now unbounded for async tools** — N concurrent calls in JS means N concurrent tasks in Python. What follows is about bounding that.

---

## The constraint that shapes every option

The server's per-tool timeout starts at **dispatch**, not at execution: [`execute_callback`](../crates/pctx_session_server/src/state/ws_manager.rs#L146) sends the request, *then* starts `tokio::time::timeout`. Any client-side queue silently spends the tool's timeout budget waiting.

Measured — 40 sync calls × 2s work, `tool_timeout_secs=5`:

```
starts: 14 @ 0.19s | 14 @ 2.2s | 12 @ 4.2s
result: ok=28, failed=12
        'Tool `tools__slow_sync` timed out after 5s'
```

12 calls died on a 5s deadline while doing 2s of work, purely from queue wait. **A concurrency cap converts a throughput problem into hard failures.** Any cap has to be introduced with this in mind.

## Where we are today

Measured on a 10-core machine, 40-way fan-out:

| | peak in-flight | wall |
|---|---|---|
| async tools | 40 (unbounded) | 0.66s |
| sync tools | **14** | 1.68s |

The 14 is `min(32, cpu_count + 4)` — asyncio's default `ThreadPoolExecutor`, inherited via `to_thread`. So there is **already a cap on the sync path**, it is just accidental and machine-dependent: 14 here, 6 on a 2-core CI box, moving the timeout knee with the hardware.

---

## Options

| Option | How | Pros | Cons |
|---|---|---|---|
| **A. Explicit sync executor** | Dedicated `ThreadPoolExecutor` with a configured size instead of asyncio's default | Replaces an invisible machine-dependent cap with a stated one; no behaviour change where `cpu+4 == size` | Doesn't bound async tools |
| **B. Opt-in `max_concurrent_tools`** | Semaphore around tool dispatch, default `None` (unbounded) | Available for rate-limited upstreams; preserves current behaviour by default | Queue wait burns the server timeout (above); one slow tool blocks unrelated fast ones |
| **C. Server sends its deadline** | Include the deadline in `ExecuteToolRequest` | Client can fail fast with a real reason, or drop work it can't service in time; makes caps *safe* rather than merely possible | Protocol + server change; both sides must ship |
| **D. Per-tool limits in the tool body** | Caller puts a semaphore inside their own tool function | Correctly scoped per upstream; no client change | Not discoverable; every caller reimplements it |

### Why not a global semaphore on its own
A single cap means a slow Excel call blocks an unrelated fast Graph call. A client-level cap is a **resource safety valve** (sockets, memory, threads), not a throttle. Rate limiting is per-upstream and belongs where the upstream is known — **D**, or **B** scoped per tool rather than globally.

---

## Recommendation
Ship the current fix as-is; it is a strict improvement and adds no cap where none existed.

Then, cheapest-first:
1. **A** — make the sync cap explicit. It's the one cap that already exists and already bites, silently and differently per machine.
2. **B**, defaulting to unbounded. Given the timeout coupling, unbounded is the correct default: everything starts immediately and gets its full budget. Offer the knob to callers protecting an upstream, and document that `tool_timeout_secs` must cover *queue wait plus call duration*, not just call duration.
3. **C** if caps become load-bearing. Without the deadline, a queued client is blind to a clock already running against it, and overload surfaces as mystery timeouts.

Treat **D** as the documented answer for per-upstream rate limiting regardless of whether B lands.
