//! A dedicated V8 worker thread that runs all JsRuntime work on a single OS thread
//! via `tokio::task::LocalSet`, avoiding cross-thread V8 platform races.
//!
//! Requests are processed sequentially — only one execution is active at a time.
//! This is required because deno_core does not support interleaving multiple
//! `JsRuntime` instances on the same thread (even cooperatively via async).

use tokio::sync::{mpsc, oneshot};
use tracing::debug;

use crate::{DenoExecutorError, ExecuteOptions, ExecuteResult, Result};

struct ExecutionRequest {
    code: String,
    options: ExecuteOptions,
    response_tx: oneshot::Sender<Result<ExecuteResult>>,
}

pub(crate) struct V8WorkerPool {
    sender: mpsc::UnboundedSender<ExecutionRequest>,
}

impl V8WorkerPool {
    /// Spawn a dedicated OS thread running a single-threaded tokio runtime + `LocalSet`.
    ///
    /// All V8 isolate creation and usage is confined to this thread.
    /// Requests are processed one at a time to avoid overlapping `JsRuntime` lifetimes.
    pub(crate) fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<ExecutionRequest>();

        std::thread::Builder::new()
            .name("v8-worker".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create tokio runtime for V8 worker");

                let local = tokio::task::LocalSet::new();

                rt.block_on(local.run_until(async {
                    while let Some(req) = rx.recv().await {
                        debug!("V8 worker received execution request");
                        let result = crate::execute_inner(&req.code, req.options).await;
                        let _ = req.response_tx.send(result);
                    }
                    debug!("V8 worker channel closed, shutting down");
                }));
            })
            .expect("Failed to spawn V8 worker thread");

        V8WorkerPool { sender: tx }
    }

    pub(crate) async fn execute(
        &self,
        code: &str,
        options: ExecuteOptions,
    ) -> Result<ExecuteResult> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(ExecutionRequest {
                code: code.to_string(),
                options,
                response_tx: tx,
            })
            .map_err(|_| DenoExecutorError::InternalError("V8 worker shut down".into()))?;

        rx.await
            .map_err(|_| DenoExecutorError::InternalError("V8 worker dropped request".into()))?
    }
}
