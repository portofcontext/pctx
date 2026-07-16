use std::{collections::HashMap, sync::Arc};

use pctx_code_mode::{CodeMode, ExecutorPool};

use crate::{
    LocalBackend,
    metadata::{NoopMetadata, SessionMetadata},
    state::{backend::PctxSessionBackend, ws_manager::WsManager},
};

pub(crate) mod backend;
pub(crate) mod ws_manager;

/// Shared application state
#[derive(Clone)]
pub struct AppState<B: PctxSessionBackend> {
    pub ws_manager: Arc<WsManager>,
    pub backend: Arc<B>,
    pub metadata: Arc<dyn SessionMetadata>,
    /// Snapshot of the process environment at server startup, passed opaquely
    /// to [`SessionMetadata`] hooks. Implementations read whatever keys they
    /// need; the `pctx` binary has no opinion on their contents.
    pub env: Arc<HashMap<String, String>>,
    /// Optional process pool for parallel TypeScript execution.
    /// When `None`, execution falls back to the in-process single-threaded path.
    pub pool: Option<Arc<ExecutorPool>>,
}

impl<B: PctxSessionBackend> AppState<B> {
    pub fn new(backend: B) -> Self {
        Self {
            ws_manager: Arc::default(),
            backend: Arc::new(backend),
            metadata: Arc::new(NoopMetadata),
            env: Arc::new(std::env::vars().collect()),
            pool: None,
        }
    }

    /// Attach a custom [`SessionMetadata`] implementation.
    ///
    /// The infra layer calls this to inject routing metadata (e.g. a
    /// Redis-backed implementation that maps session IDs to pod names).
    #[must_use]
    pub fn with_metadata(mut self, metadata: impl SessionMetadata + 'static) -> Self {
        self.metadata = Arc::new(metadata);
        self
    }

    /// Attach an [`ExecutorPool`] for parallel TypeScript execution.
    ///
    /// When set, every `execute_typescript` / `execute_bash` call will be
    /// dispatched to a worker subprocess rather than running in-process.
    #[must_use]
    pub fn with_executor_pool(mut self, pool: Arc<ExecutorPool>) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Re-attach the pool to a freshly-fetched [`CodeMode`] instance.
    ///
    /// Because `executor` is `#[serde(skip)]`, it is always `None` after
    /// deserialisation from the backend.  Call this after every
    /// `backend.get()` before passing `CodeMode` to an execution function.
    pub fn attach_pool(&self, code_mode: CodeMode) -> CodeMode {
        if let Some(pool) = &self.pool {
            code_mode.with_executor_pool(pool.clone())
        } else {
            code_mode
        }
    }
}

impl AppState<LocalBackend> {
    pub fn new_local() -> Self {
        Self::new(LocalBackend::default())
    }
}
