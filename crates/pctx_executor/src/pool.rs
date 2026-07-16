/// Process pool for parallel TypeScript execution.
///
/// [`ExecutorPool`] manages N worker sub-processes, each with its own V8
/// platform. Requests are dispatched round-robin so that N executions can run
/// truly in parallel, eliminating the contention of the process-wide
/// `V8_MUTEX` when all callers share a single process.
///
/// # Callback proxying
///
/// The worker process cannot hold Rust closures, so when TypeScript code calls
/// a callback registered in the parent's [`PctxRegistry`], the worker sends a
/// [`CallbackRequest`] IPC message and suspends.  The pool's `execute()` loop
/// receives that message, invokes the real callback via the parent's registry,
/// and sends the result back as a [`CallbackResponse`].  The worker resumes
/// transparently.
///
/// # Connection pool caching
///
/// MCP connections made *inside* a worker are independent of the parent
/// process's connection pool.  The [`ExecuteResult`] returned by
/// [`ExecutorPool::execute`] contains the *original* registry that was passed
/// in (so pool caching by the session / MCP server layers continues to work
/// for parent-side connections), but the worker will reconnect on every
/// request.  Session-affinity routing is a planned follow-up to address this.
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::SystemTime,
};

use pctx_registry::PctxRegistry;
use tokio::{
    io::{AsyncWriteExt, BufReader, BufWriter},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    DenoExecutorError, ExecuteOptions, ExecuteResult,
    events::{ExecutionEvent, ExecutionTrace},
    ipc::{read_msg, write_msg},
    protocol::{CallbackResponse, ExecuteRequest, WorkerMessage, WorkerRequest},
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Configuration for an [`ExecutorPool`].
pub struct PoolConfig {
    /// Number of worker processes to spawn (one V8 platform each).
    pub worker_count: usize,
    /// Path to the `pctx_worker` binary.
    pub worker_binary: PathBuf,
}

impl PoolConfig {
    /// Build a config that locates `pctx_worker` as a sibling of the current
    /// executable (i.e., same `target/debug` or `target/release` directory).
    ///
    /// # Errors
    /// Returns an error if `std::env::current_exe()` fails.
    pub fn from_current_exe(worker_count: usize) -> std::io::Result<Self> {
        let exe = std::env::current_exe()?;
        let dir = exe.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "current executable has no parent directory",
            )
        })?;
        Ok(Self {
            worker_count,
            worker_binary: dir.join("pctx_worker"),
        })
    }
}

/// A pool of worker processes that execute TypeScript in parallel.
///
/// Construct with [`ExecutorPool::new`], then call [`ExecutorPool::execute`]
/// as a drop-in replacement for [`crate::execute`].
pub struct ExecutorPool {
    workers: Vec<Arc<Mutex<WorkerHandle>>>,
    next: AtomicUsize,
}

impl std::fmt::Debug for ExecutorPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorPool")
            .field("worker_count", &self.workers.len())
            .finish_non_exhaustive()
    }
}

impl ExecutorPool {
    /// Spawn all worker processes and wait for each to signal readiness.
    ///
    /// # Errors
    /// Returns an error if any worker fails to start or does not send a
    /// `Ready` message within a reasonable time.
    pub async fn new(config: PoolConfig) -> std::io::Result<Self> {
        let mut workers = Vec::with_capacity(config.worker_count);
        for i in 0..config.worker_count {
            let handle = spawn_worker(&config.worker_binary).await.map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("failed to spawn worker {i}: {e}"),
                )
            })?;
            info!(worker = i, "Worker process ready");
            workers.push(Arc::new(Mutex::new(handle)));
        }
        Ok(Self {
            workers,
            next: AtomicUsize::new(0),
        })
    }

    /// Execute TypeScript code on the next available worker.
    ///
    /// This is a drop-in replacement for [`crate::execute`]: same inputs, same
    /// output.  Callback invocations are proxied back to the parent's registry
    /// transparently.
    ///
    /// # Errors
    /// Returns [`DenoExecutorError::InternalError`] if the IPC channel fails
    /// (e.g., the worker process crashed).
    pub async fn execute(
        &self,
        code: &str,
        options: ExecuteOptions,
    ) -> crate::Result<ExecuteResult> {
        let worker_idx = self.next.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let mut worker = self.workers[worker_idx].lock().await;

        let started_at = SystemTime::now();
        let request_id = Uuid::new_v4();

        let all_tool_ids = options.registry.ids();

        let req = ExecuteRequest {
            request_id,
            code: code.to_string(),
            all_tool_ids,
        };

        debug!(
            worker = worker_idx,
            %request_id,
            code_len = code.len(),
            "Dispatching to worker",
        );

        // Send the execute request.
        write_msg(&mut worker.stdin, &WorkerRequest::Execute(req))
            .await
            .map_err(|e| DenoExecutorError::InternalError(format!("IPC write: {e}")))?;
        worker
            .stdin
            .flush()
            .await
            .map_err(|e| DenoExecutorError::InternalError(format!("IPC flush: {e}")))?;

        // Drive the message loop until the worker returns a result.
        // Callback requests are intercepted and proxied here.
        let result_msg = loop {
            let msg: WorkerMessage = read_msg(&mut worker.stdout)
                .await
                .map_err(|e| DenoExecutorError::InternalError(format!("IPC read: {e}")))?;

            match msg {
                WorkerMessage::ExecuteResult(r) => break r,

                WorkerMessage::CallbackRequest(cb) => {
                    debug!(
                        callback_id = %cb.callback_id,
                        call_id = %cb.callback_call_id,
                        "Proxying callback to parent registry",
                    );
                    let args_obj = cb.args.as_ref().and_then(|v| v.as_object().cloned());
                    let result = options
                        .registry
                        .invoke(&cb.callback_id, args_obj)
                        .await
                        .map_err(|e| e.to_string());

                    let resp = WorkerRequest::CallbackResponse(CallbackResponse {
                        callback_call_id: cb.callback_call_id,
                        result,
                    });
                    write_msg(&mut worker.stdin, &resp)
                        .await
                        .map_err(|e| DenoExecutorError::InternalError(format!("IPC write: {e}")))?;
                    worker
                        .stdin
                        .flush()
                        .await
                        .map_err(|e| DenoExecutorError::InternalError(format!("IPC flush: {e}")))?;
                }

                WorkerMessage::Ready => {
                    warn!("Received unexpected Ready message during execution; ignoring");
                }
            }
        };

        let ended_at = SystemTime::now();

        debug!(
            worker = worker_idx,
            %request_id,
            success = result_msg.success,
            "Worker execution complete",
        );

        Ok(reconstruct_result(
            result_msg,
            options.registry,
            code,
            started_at,
            ended_at,
        ))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct WorkerHandle {
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// Kept alive so the child process is not killed on drop.
    _child: Child,
}

/// Spawn one worker process and wait for the `Ready` handshake.
async fn spawn_worker(binary: &PathBuf) -> std::io::Result<WorkerHandle> {
    let mut child = Command::new(binary)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("failed to spawn {}: {e}", binary.display()),
            )
        })?;

    let stdin = BufWriter::new(child.stdin.take().expect("stdin piped"));
    let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));

    let mut handle = WorkerHandle {
        stdin,
        stdout,
        _child: child,
    };

    // Wait for the worker to finish V8 platform initialisation.
    let first_msg: WorkerMessage = read_msg(&mut handle.stdout).await.map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("worker did not send Ready signal: {e}"),
        )
    })?;

    if !matches!(first_msg, WorkerMessage::Ready) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "worker sent unexpected first message (expected Ready)",
        ));
    }

    Ok(handle)
}


/// Reconstruct an [`ExecuteResult`] from the worker's response message.
///
/// The original registry is returned unchanged so the caller's connection-pool
/// caching continues to work for parent-side MCP connections.
fn reconstruct_result(
    msg: crate::protocol::ExecuteResultMsg,
    original_registry: PctxRegistry,
    code: &str,
    started_at: SystemTime,
    ended_at: SystemTime,
) -> ExecuteResult {
    // The events vec arriving from the worker already includes the TypeCheck
    // event and all registry events, sorted by started_at.
    let events: Vec<ExecutionEvent> = msg.events;

    ExecuteResult {
        success: msg.success,
        diagnostics: msg.diagnostics,
        runtime_error: msg.runtime_error,
        output: msg.output,
        stdout: msg.stdout,
        stderr: msg.stderr,
        registry: original_registry,
        trace: ExecutionTrace {
            code: code.to_string(),
            started_at,
            ended_at,
            events,
        },
    }
}
