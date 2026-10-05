//! Guards for runtime semantics that depend on `deno_core` / V8 internals.
//!
//! These pin down behavior that an upgrade of the Deno stack could silently
//! change: async op scheduling (tool calls and timers), event loop draining,
//! error propagation, and the ICU / Temporal / WebAssembly builtins.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use pctx_code_execution_runtime::PctxRegistry;
use serde_json::{Value, json};

use super::serial;
use crate::{ExecuteOptions, ExecuteResult, execute};

type CallLog = Arc<Mutex<Vec<String>>>;

/// Registry with a delayed tool, an immediate tool and a failing tool.
/// Every call start/end is recorded in `log`.
fn registry(log: &CallLog) -> PctxRegistry {
    let registry = PctxRegistry::default();

    let l = log.clone();
    registry
        .add_callback(
            "probe.slow",
            Arc::new(move |args: Option<Value>| {
                let l = l.clone();
                Box::pin(async move {
                    let args = args.unwrap_or_default();
                    let tag = args["tag"].as_str().unwrap_or("?").to_string();
                    let ms = args["ms"].as_u64().unwrap_or(0);
                    l.lock().unwrap().push(format!("start:{tag}"));
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    l.lock().unwrap().push(format!("end:{tag}"));
                    Ok(json!({ "tag": tag }))
                })
            }),
        )
        .expect("callback registration should succeed");

    let l = log.clone();
    registry
        .add_callback(
            "probe.sync",
            Arc::new(move |args: Option<Value>| {
                let l = l.clone();
                Box::pin(async move {
                    let tag = args.unwrap_or_default()["tag"]
                        .as_str()
                        .unwrap_or("?")
                        .to_string();
                    l.lock().unwrap().push(format!("sync:{tag}"));
                    Ok(json!(tag))
                })
            }),
        )
        .expect("callback registration should succeed");

    registry
        .add_callback(
            "probe.fail",
            Arc::new(move |_args: Option<Value>| {
                Box::pin(async move { Err("tool exploded".to_string()) })
            }),
        )
        .expect("callback registration should succeed");

    registry
}

async fn run(code: &str, log: &CallLog) -> ExecuteResult {
    tokio::time::timeout(
        Duration::from_secs(30),
        execute(code, ExecuteOptions::new().with_registry(registry(log))),
    )
    .await
    .expect("execution should not hang")
    .expect("execution should not error internally")
}

fn assert_ok(result: &ExecuteResult) {
    assert!(
        result.success,
        "execution failed: runtime_error={:?} diagnostics={:?}",
        result.runtime_error, result.diagnostics
    );
}

#[serial]
#[tokio::test]
async fn test_tool_call_resolves_after_microtasks() {
    let log = CallLog::default();
    let result = run(
        r#"
async function test() {
    const order: string[] = [];
    const p = invokeInternal({ name: "probe.sync", arguments: { tag: "A" } }).then(() => order.push("tool-resolved"));
    order.push("after-call");
    Promise.resolve().then(() => order.push("microtask"));
    queueMicrotask(() => order.push("queueMicrotask"));
    await p;
    order.push("after-await");
    return order;
}
export default await test();
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!([
            "after-call",
            "microtask",
            "queueMicrotask",
            "tool-resolved",
            "after-await"
        ]))
    );
}

#[serial]
#[tokio::test]
async fn test_parallel_tool_calls_run_concurrently() {
    let log = CallLog::default();
    let result = run(
        r#"
async function test() {
    const t0 = Date.now();
    const results = await Promise.all([
        invokeInternal({ name: "probe.slow", arguments: { tag: "a", ms: 300 } }),
        invokeInternal({ name: "probe.slow", arguments: { tag: "b", ms: 100 } }),
        invokeInternal({ name: "probe.slow", arguments: { tag: "c", ms: 200 } }),
    ]);
    return { results, elapsed: Date.now() - t0 };
}
export default await test();
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    let output = result.output.expect("should have output");
    assert_eq!(
        output["results"],
        json!([{ "tag": "a" }, { "tag": "b" }, { "tag": "c" }]),
        "Promise.all should keep input order"
    );
    let elapsed = output["elapsed"].as_u64().unwrap();
    assert!(
        elapsed < 550,
        "tool calls should overlap (took {elapsed}ms, sequential would be ~600ms)"
    );

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["start:a", "start:b", "start:c", "end:b", "end:c", "end:a"],
        "all calls should start before any finishes"
    );
}

#[serial]
#[tokio::test]
async fn test_sequential_tool_calls_do_not_overlap() {
    let log = CallLog::default();
    let result = run(
        r#"
async function test() {
    const out: unknown[] = [];
    for (const tag of ["x", "y", "z"]) {
        out.push(await invokeInternal({ name: "probe.slow", arguments: { tag, ms: 20 } }));
    }
    return out;
}
export default await test();
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!([{ "tag": "x" }, { "tag": "y" }, { "tag": "z" }]))
    );
    assert_eq!(
        log.lock().unwrap().clone(),
        vec!["start:x", "end:x", "start:y", "end:y", "start:z", "end:z"]
    );
}

#[serial]
#[tokio::test]
async fn test_tool_errors_are_catchable_rejections() {
    let log = CallLog::default();
    let result = run(
        r#"
async function test() {
    const settled = await Promise.allSettled([
        invokeInternal({ name: "probe.slow", arguments: { tag: "ok", ms: 10 } }),
        invokeInternal({ name: "probe.fail", arguments: {} }),
    ]);
    let caught: string | null = null;
    try {
        await invokeInternal({ name: "probe.fail", arguments: {} });
    } catch (e) {
        caught = e instanceof Error ? e.message : String(e);
    }
    const winner = await Promise.race([
        invokeInternal({ name: "probe.slow", arguments: { tag: "slow", ms: 200 } }),
        invokeInternal({ name: "probe.slow", arguments: { tag: "fast", ms: 10 } }),
    ]);
    return { settled: settled.map((s) => s.status), caught, winner };
}
export default await test();
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    let output = result.output.expect("should have output");
    assert_eq!(output["settled"], json!(["fulfilled", "rejected"]));
    assert!(
        output["caught"].as_str().unwrap().contains("tool exploded"),
        "unexpected error message: {}",
        output["caught"]
    );
    assert_eq!(output["winner"], json!({ "tag": "fast" }));
}

#[serial]
#[tokio::test]
async fn test_uncaught_tool_error_fails_execution() {
    let log = CallLog::default();
    let result = run(
        r#"
async function test() {
    return await invokeInternal({ name: "probe.fail", arguments: {} });
}
export default await test();
"#,
        &log,
    )
    .await;

    assert!(!result.success);
    let error = result.runtime_error.expect("should have runtime error");
    assert!(
        error.message.contains("tool exploded"),
        "unexpected error message: {}",
        error.message
    );
}

#[serial]
#[tokio::test]
async fn test_timer_ordering() {
    let log = CallLog::default();
    let result = run(
        r#"
async function test() {
    const order: string[] = [];
    setTimeout(() => order.push("t0"), 0);
    queueMicrotask(() => order.push("micro"));
    Promise.resolve().then(() => order.push("then"));
    order.push("sync");
    await new Promise((r) => setTimeout(r, 20));
    setTimeout(() => order.push("t30"), 30);
    setTimeout(() => order.push("t10"), 10);
    setTimeout(() => order.push("t20"), 20);
    await new Promise((r) => setTimeout(r, 80));
    return order;
}
export default await test();
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!(["sync", "micro", "then", "t0", "t10", "t20", "t30"]))
    );
}

#[serial]
#[tokio::test]
async fn test_set_interval_and_clear() {
    let log = CallLog::default();
    let result = run(
        r"
async function test() {
    let n = 0;
    await new Promise<void>((resolve) => {
        const id = setInterval(() => {
            n++;
            if (n === 5) {
                clearInterval(id);
                resolve();
            }
        }, 5);
    });
    return n;
}
export default await test();
",
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(result.output, Some(json!(5)));
}

#[serial]
#[tokio::test]
async fn test_event_loop_drains_work_scheduled_after_export() {
    let log = CallLog::default();
    let result = run(
        r#"
setTimeout(() => console.log("late timer fired"), 50);
invokeInternal({ name: "probe.slow", arguments: { tag: "late", ms: 50 } }).then((v) => console.log("late tool", JSON.stringify(v)));
export default 1;
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    assert!(
        result.stdout.contains("late timer fired"),
        "stdout: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains(r#"late tool {"tag":"late"}"#),
        "stdout: {}",
        result.stdout
    );
    assert_eq!(log.lock().unwrap().clone(), vec!["start:late", "end:late"]);
}

#[serial]
#[tokio::test]
async fn test_unhandled_rejection_fails_execution() {
    let log = CallLog::default();
    let result = run(
        r#"
Promise.reject(new Error("boom"));
export default 1;
"#,
        &log,
    )
    .await;

    assert!(!result.success);
    let error = result.runtime_error.expect("should have runtime error");
    assert!(error.message.contains("boom"), "{}", error.message);
}

#[serial]
#[tokio::test]
async fn test_runtime_error_includes_stack() {
    let log = CallLog::default();
    let result = run(
        r#"
function inner() { throw new TypeError("bad thing"); }
function outer() { inner(); }
outer();
"#,
        &log,
    )
    .await;

    assert!(!result.success);
    let error = result.runtime_error.expect("should have runtime error");
    assert!(
        error.message.starts_with("TypeError: bad thing"),
        "{}",
        error.message
    );
    assert!(error.message.contains("at inner"), "{}", error.message);
    assert!(error.message.contains("at outer"), "{}", error.message);
}

#[serial]
#[tokio::test]
async fn test_stack_overflow_is_catchable() {
    let log = CallLog::default();
    let result = run(
        r"
function recurse(n: number): number { return recurse(n + 1) + 1; }
let msg = '';
try { recurse(0); } catch (e) { msg = (e as Error).constructor.name + ': ' + (e as Error).message; }
export default msg;
",
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!("RangeError: Maximum call stack size exceeded"))
    );
}

#[serial]
#[tokio::test]
async fn test_intl_uses_bundled_icu_data() {
    let log = CallLog::default();
    let result = run(
        r#"
const d = new Date(Date.UTC(2026, 9, 5, 12));
export default {
    nyHour: new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", hour: "numeric", hour12: false }).format(d),
    tokyoHour: new Intl.DateTimeFormat("en-US", { timeZone: "Asia/Tokyo", hour: "numeric", hour12: false }).format(d),
    plural: new Intl.PluralRules("ar").select(3),
    relative: new Intl.RelativeTimeFormat("fr").format(-2, "day"),
    sorted: ["ä", "a", "z"].sort(new Intl.Collator("de").compare),
    turkishUpper: "i".toLocaleUpperCase("tr"),
    japaneseEra: new Intl.DateTimeFormat("ja-JP-u-ca-japanese", { era: "long", year: "numeric", timeZone: "UTC" }).format(d),
};
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!({
            "nyHour": "08",
            "tokyoHour": "21",
            "plural": "few",
            "relative": "il y a 2 jours",
            "sorted": ["a", "ä", "z"],
            "turkishUpper": "İ",
            "japaneseEra": "令和8年",
        }))
    );
}

#[serial]
#[tokio::test]
async fn test_temporal_available() {
    let log = CallLog::default();
    let result = run(
        r#"
const T = (globalThis as any).Temporal;
export default {
    plain: T.PlainDate.from("2026-10-05").add({ months: 1 }).toString(),
    dstGap: T.ZonedDateTime.from("2026-03-08T01:30[America/New_York]").add({ hours: 1 }).toString(),
    until: T.PlainDate.from("2026-01-31").until("2026-03-01", { largestUnit: "month" }).toString(),
    now: typeof T.Now.instant().epochMilliseconds,
};
"#,
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!({
            "plain": "2026-11-05",
            "dstGap": "2026-03-08T03:30:00-04:00[America/New_York]",
            "until": "P1M1D",
            "now": "number",
        }))
    );
}

/// Async WebAssembly compilation relies on V8 platform tasks being pumped by
/// the event loop.
#[serial]
#[tokio::test]
async fn test_webassembly_sync_and_async() {
    let log = CallLog::default();
    let result = run(
        r"
// (module (func (export \x22add\x22) (param i32 i32) (result i32) local.get 0 local.get 1 i32.add))
const bytes = new Uint8Array([0,97,115,109,1,0,0,0,1,7,1,96,2,127,127,1,127,3,2,1,0,7,7,1,3,97,100,100,0,0,10,9,1,7,0,32,0,32,1,106,11]);
async function test() {
    const W = (globalThis as any).WebAssembly;
    const sync = new W.Instance(new W.Module(bytes)).exports.add(2, 3);
    const { instance } = await W.instantiate(bytes);
    const compiled = await W.compile(bytes);
    return { sync, instantiate: instance.exports.add(4, 5), compile: new W.Instance(compiled).exports.add(1, 1) };
}
export default await test();
",
        &log,
    )
    .await;

    assert_ok(&result);
    assert_eq!(
        result.output,
        Some(json!({ "sync": 5, "instantiate": 9, "compile": 2 }))
    );
}

/// Like `concurrent_v8_stress`, but each execution also exercises async ops
/// (tool calls, timers) and async WebAssembly compilation.
#[test]
fn test_concurrent_executions_with_async_ops() {
    let handles: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async {
                    for j in 0..3 {
                        let log = CallLog::default();
                        let code = format!(
                            r#"
const bytes = new Uint8Array([0,97,115,109,1,0,0,0,1,7,1,96,2,127,127,1,127,3,2,1,0,7,7,1,3,97,100,100,0,0,10,9,1,7,0,32,0,32,1,106,11]);
async function test() {{
    const {{ instance }} = await (globalThis as any).WebAssembly.instantiate(bytes);
    await new Promise((r) => setTimeout(r, 5));
    const [a, b] = await Promise.all([
        invokeInternal({{ name: "probe.slow", arguments: {{ tag: "{i}-{j}", ms: 10 }} }}),
        invokeInternal({{ name: "probe.sync", arguments: {{ tag: "{i}-{j}" }} }}),
    ]);
    return {{ sum: instance.exports.add({i}, {j}), a, b }};
}}
export default await test();
"#
                        );
                        let result = run(&code, &log).await;
                        assert_ok(&result);
                        assert_eq!(
                            result.output,
                            Some(json!({
                                "sum": i + j,
                                "a": { "tag": format!("{i}-{j}") },
                                "b": format!("{i}-{j}"),
                            })),
                            "iteration {i}_{j}"
                        );
                    }
                });
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
