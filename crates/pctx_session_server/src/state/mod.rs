use std::sync::Arc;

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
}

impl<B: PctxSessionBackend> AppState<B> {
    pub fn new(backend: B) -> Self {
        Self {
            ws_manager: Arc::default(),
            backend: Arc::new(backend),
            metadata: Arc::new(NoopMetadata),
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
}

impl AppState<LocalBackend> {
    pub fn new_local() -> Self {
        Self::new(LocalBackend::default())
    }
}
