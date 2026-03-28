use std::collections::HashMap;

use async_trait::async_trait;
use uuid::Uuid;

/// Hook interface for session lifecycle events.
///
/// Implement this trait to attach external behaviour to session creation and
/// closure — routing table updates, audit logs, analytics, etc.  The built-in
/// [`NoopMetadata`] is used by default and does nothing.
///
/// The `env` parameter is a snapshot of the process environment at server
/// startup. Implementations can read whatever keys they need without the
/// `pctx` binary needing to know those keys exist.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use async_trait::async_trait;
/// use pctx_session_server::SessionMetadata;
/// use uuid::Uuid;
///
/// struct MyMetadata;
///
/// #[async_trait]
/// impl SessionMetadata for MyMetadata {
///     async fn on_session_created(&self, session_id: Uuid, env: &HashMap<String, String>) {
///         let target = env.get("ROUTING_TARGET").map(String::as_str).unwrap_or("unknown");
///         println!("session {session_id} created, routing target: {target}");
///     }
///     async fn on_session_closed(&self, session_id: Uuid, _env: &HashMap<String, String>) {
///         println!("session {session_id} closed");
///     }
/// }
///
/// let state = AppState::new_local().with_metadata(MyMetadata);
/// ```
#[async_trait]
pub trait SessionMetadata: Send + Sync {
    /// Called immediately after a session is inserted into the backend.
    async fn on_session_created(&self, session_id: Uuid, env: &HashMap<String, String>);

    /// Called immediately after a session is removed from the backend.
    async fn on_session_closed(&self, session_id: Uuid, env: &HashMap<String, String>);
}

/// No-op implementation — the default for local development and testing.
#[derive(Debug, Default, Clone)]
pub struct NoopMetadata;

#[async_trait]
impl SessionMetadata for NoopMetadata {
    async fn on_session_created(&self, _session_id: Uuid, _env: &HashMap<String, String>) {}
    async fn on_session_closed(&self, _session_id: Uuid, _env: &HashMap<String, String>) {}
}
