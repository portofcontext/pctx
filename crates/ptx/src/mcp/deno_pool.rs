use deno_executor::ExecuteResult;
use tokio::sync::{mpsc, oneshot};

/// A job sent to the Deno worker
struct DenoJob {
    code: String,
    response: oneshot::Sender<ExecuteResult>,
}

/// Deno executor that runs on a dedicated thread
///
/// This wrapper ensures V8 isolates stay on a single thread.
/// Each executor creates a dedicated OS thread with its own tokio runtime and Deno worker.
#[derive(Clone)]
pub(crate) struct DenoExecutor {
    sender: mpsc::Sender<DenoJob>,
}

impl DenoExecutor {
    /// Create a new Deno executor on a dedicated thread
    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn new(allowed_hosts: Option<Vec<String>>) -> Self {
        let (tx, mut rx) = mpsc::channel::<DenoJob>(100);
        let allowed_hosts_clone = allowed_hosts.clone();

        // Spawn dedicated thread for Deno/V8
        std::thread::spawn(move || {
            // Install default crypto provider for rustls (required for TLS/HTTPS)
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

            // Create single-threaded tokio runtime on this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Deno runtime");

            rt.block_on(async move {
                // Process jobs sequentially on this thread
                while let Some(job) = rx.recv().await {
                    let result = deno_executor::execute(&job.code, allowed_hosts_clone.clone())
                        .await
                        .unwrap_or_else(|e| ExecuteResult {
                            success: false,
                            diagnostics: vec![],
                            runtime_error: Some(deno_executor::RuntimeError {
                                message: e.to_string(),
                                stack: None,
                            }),
                            output: None,
                            stdout: String::new(),
                            stderr: String::new(),
                        });

                    // Send result back (ignore if receiver dropped)
                    let _ = job.response.send(result);
                }
            });
        });

        Self { sender: tx }
    }

    /// Execute TypeScript code
    pub(crate) async fn execute(&self, code: String) -> Result<ExecuteResult, &'static str> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(DenoJob { code, response: tx })
            .await
            .map_err(|_| "Deno executor shut down")?;

        rx.await.map_err(|_| "Deno executor dropped response")
    }
}
