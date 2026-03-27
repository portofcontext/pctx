use async_trait::async_trait;
use uuid::Uuid;

/// Hook interface for session lifecycle events.
///
/// Implement this trait to attach external behaviour to session creation and
/// closure — routing table updates, audit logs, analytics, etc.  The built-in
/// [`NoopMetadata`] is used by default and does nothing.
///
/// # Example
///
/// ```rust,ignore
/// use async_trait::async_trait;
/// use pctx_session_server::metadata::SessionMetadata;
/// use uuid::Uuid;
///
/// struct MyMetadata;
///
/// #[async_trait]
/// impl SessionMetadata for MyMetadata {
///     async fn on_session_created(&self, session_id: Uuid) {
///         println!("session created: {session_id}");
///     }
///     async fn on_session_closed(&self, session_id: Uuid) {
///         println!("session closed: {session_id}");
///     }
/// }
///
/// let state = AppState::new_local().with_metadata(MyMetadata);
/// ```
#[async_trait]
pub trait SessionMetadata: Send + Sync {
    /// Called immediately after a session is inserted into the backend.
    async fn on_session_created(&self, session_id: Uuid);

    /// Called immediately after a session is removed from the backend.
    async fn on_session_closed(&self, session_id: Uuid);
}

/// No-op implementation — the default for local development and testing.
///
/// Compiles away entirely in release builds when the compiler can see through
/// the `Arc<dyn SessionMetadata>` indirection.
#[derive(Debug, Default, Clone)]
pub struct NoopMetadata;

#[async_trait]
impl SessionMetadata for NoopMetadata {
    async fn on_session_created(&self, _session_id: Uuid) {}
    async fn on_session_closed(&self, _session_id: Uuid) {}
}
