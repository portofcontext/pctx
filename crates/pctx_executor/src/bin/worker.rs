/// Worker process binary for the `pctx_executor` process pool.
///
/// Each worker owns an exclusive V8 platform and processes one
/// [`ExecuteRequest`] at a time, eliminating V8's cross-thread sharing
/// constraint that serialises execution in the parent process.
///
/// # Lifecycle
/// 1. Writes `{"type":"ready"}` to stdout once V8 is initialised.
/// 2. Reads one `ExecuteRequest` from stdin.
/// 3. Builds a [`PctxRegistry`] with:
///    - Real MCP connections (fresh per request).
///    - Synthetic proxy callbacks that bounce to the parent via IPC.
/// 4. Runs `pctx_executor::execute()` while concurrently reading
///    `CallbackResponse` messages from stdin (driven by `tokio::select!`).
/// 5. Writes the `ExecuteResult` back to stdout and returns to step 2.
use std::{collections::HashMap, sync::Arc};

use pctx_executor::{
    DenoExecutorError, ExecuteOptions, ExecuteResult,
    ipc::{read_msg, write_msg},
    protocol::{CallbackRequest, ExecuteResultMsg, WorkerMessage, WorkerRequest},
};
use pctx_registry::{CallbackFn, PctxRegistry};
use serde_json::Value;
use tokio::{
    io::{AsyncWriteExt, BufReader, BufWriter, stdin, stdout},
    sync::{Mutex, oneshot},
};
use uuid::Uuid;

/// Pending callback invocations: `call_id` → `oneshot::Sender`.
type Pending = Arc<Mutex<HashMap<Uuid, oneshot::Sender<Result<Value, String>>>>>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Wrap stdout in Arc<Mutex<>> so synthetic callbacks can write to it
    // concurrently with the main loop (all on the same single thread, so the
    // mutex never actually contends, but the types require it).
    let stdout_shared = Arc::new(Mutex::new(BufWriter::new(stdout())));
    let mut stdin = BufReader::new(stdin());

    // Signal ready – this also triggers the first V8 platform init via the
    // LazyLock inside pctx_executor when execute() is first called.
    send_msg(&stdout_shared, &WorkerMessage::Ready).await;

    loop {
        let req: WorkerRequest = match read_msg(&mut stdin).await {
            Ok(r) => r,
            Err(_) => break, // EOF = parent closed the pipe; exit cleanly.
        };

        let WorkerRequest::Execute(exec_req) = req else {
            // Unexpected message type before an Execute request – skip.
            continue;
        };

        let request_id = exec_req.request_id;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));

        let registry = build_registry(&exec_req, pending.clone(), stdout_shared.clone());
        let code = exec_req.code.clone();

        let result_msg =
            run_execution(request_id, code, registry, &mut stdin, &pending).await;

        send_msg(&stdout_shared, &WorkerMessage::ExecuteResult(result_msg)).await;
    }
}

// ---------------------------------------------------------------------------
// Core execution loop
// ---------------------------------------------------------------------------

/// Drive `execute()` and read `CallbackResponse` messages from stdin
/// concurrently on the single-thread runtime using `tokio::select!`.
///
/// When a synthetic callback suspends waiting for a parent response, the
/// `select!` reads that response from stdin and satisfies the `oneshot`
/// channel, which wakes the Deno event loop.
async fn run_execution(
    request_id: Uuid,
    code: String,
    registry: PctxRegistry,
    stdin: &mut BufReader<tokio::io::Stdin>,
    pending: &Pending,
) -> ExecuteResultMsg {
    let opts = ExecuteOptions::new().with_registry(registry);
    let exec_fut = pctx_executor::execute(&code, opts);
    tokio::pin!(exec_fut);

    loop {
        tokio::select! {
            result = &mut exec_fut => {
                return to_result_msg(request_id, result);
            }
            maybe_msg = read_msg::<_, WorkerRequest>(stdin) => {
                match maybe_msg {
                    Ok(WorkerRequest::CallbackResponse(resp)) => {
                        let mut p = pending.lock().await;
                        if let Some(tx) = p.remove(&resp.callback_call_id) {
                            // Ignore send errors: the callback future may have
                            // been cancelled if execute() returned already.
                            let _ = tx.send(resp.result);
                        }
                    }
                    Ok(_) => {} // unexpected; ignore
                    Err(_) => break, // parent closed stdin
                }
            }
        }
    }

    // Reached only if stdin closed mid-execution.
    to_result_msg(
        request_id,
        Err(DenoExecutorError::InternalError(
            "parent process disconnected during execution".into(),
        )),
    )
}

// ---------------------------------------------------------------------------
// Registry construction
// ---------------------------------------------------------------------------

/// Build a [`PctxRegistry`] for this execution request.
///
/// Every tool ID — whether a Rust callback or an MCP server tool — is backed
/// by a synthetic IPC-proxy closure.  When TypeScript invokes a tool, the
/// worker sends a [`CallbackRequest`] to the parent and suspends; the parent
/// calls its own `registry.invoke()` (which uses the live session-scoped
/// connection pool) and sends back a [`CallbackResponse`].
///
/// This means the worker never opens direct MCP connections, so it always
/// benefits from the parent's already-established and session-cached pool.
fn build_registry(
    exec_req: &pctx_executor::protocol::ExecuteRequest,
    pending: Pending,
    stdout_shared: Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
) -> PctxRegistry {
    let registry = PctxRegistry::default();

    for tool_id in &exec_req.all_tool_ids {
        let id = tool_id.clone();
        let pending = pending.clone();
        let out = stdout_shared.clone();

        let cb: CallbackFn = Arc::new(move |args: Option<Value>| {
            let id = id.clone();
            let pending = pending.clone();
            let out = out.clone();

            Box::pin(async move {
                let call_id = Uuid::new_v4();
                let (tx, rx) = oneshot::channel::<Result<Value, String>>();

                {
                    let mut p = pending.lock().await;
                    p.insert(call_id, tx);
                }

                let cb_req = CallbackRequest {
                    callback_call_id: call_id,
                    callback_id: id.clone(),
                    args,
                };
                send_msg(&out, &WorkerMessage::CallbackRequest(cb_req)).await;

                rx.await
                    .unwrap_or_else(|_| Err("callback channel dropped".into()))
            })
        });

        if let Err(e) = registry.add_callback(tool_id, cb) {
            eprintln!("[pctx_worker] failed to register tool proxy {tool_id}: {e}");
        }
    }

    registry
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a message to the shared stdout and flush.
async fn send_msg<T: serde::Serialize>(
    out: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    msg: &T,
) {
    let mut guard = out.lock().await;
    write_msg(&mut *guard, msg)
        .await
        .expect("worker: write to stdout");
    guard.flush().await.expect("worker: flush stdout");
}

/// Convert an `execute()` result into the wire-format [`ExecuteResultMsg`].
fn to_result_msg(
    request_id: Uuid,
    result: pctx_executor::Result<ExecuteResult>,
) -> ExecuteResultMsg {
    match result {
        Ok(r) => ExecuteResultMsg {
            request_id,
            success: r.success,
            diagnostics: r.diagnostics,
            runtime_error: r.runtime_error,
            output: r.output,
            stdout: r.stdout,
            stderr: r.stderr,
            events: r.trace.events,
        },
        Err(e) => {
            let msg = e.to_string();
            ExecuteResultMsg {
                request_id,
                success: false,
                diagnostics: vec![],
                runtime_error: Some(pctx_executor::ExecutionError {
                    message: msg.clone(),
                    stack: None,
                }),
                output: None,
                stdout: String::new(),
                stderr: msg,
                events: vec![],
            }
        }
    }
}
