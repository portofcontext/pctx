/// Tests for custom [`SessionMetadata`] implementations that read from the
/// env snapshot passed to each hook.
mod utils;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use axum_test::TestServer;
use pctx_session_server::{AppState, LocalBackend, SessionMetadata, server::create_router};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test metadata implementation
// ---------------------------------------------------------------------------

/// Records the value of an arbitrary env key observed at each lifecycle event.
struct EnvCapture {
    key: String,
    created: Arc<Mutex<Vec<(Uuid, String)>>>,
    closed: Arc<Mutex<Vec<(Uuid, String)>>>,
}

impl EnvCapture {
    fn new(
        key: impl Into<String>,
    ) -> (
        Self,
        Arc<Mutex<Vec<(Uuid, String)>>>,
        Arc<Mutex<Vec<(Uuid, String)>>>,
    ) {
        let created = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                key: key.into(),
                created: Arc::clone(&created),
                closed: Arc::clone(&closed),
            },
            created,
            closed,
        )
    }
}

#[async_trait]
impl SessionMetadata for EnvCapture {
    async fn on_session_created(&self, session_id: Uuid, env: &HashMap<String, String>) {
        let val = env.get(&self.key).cloned().unwrap_or_default();
        self.created.lock().unwrap().push((session_id, val));
    }

    async fn on_session_closed(&self, session_id: Uuid, env: &HashMap<String, String>) {
        let val = env.get(&self.key).cloned().unwrap_or_default();
        self.closed.lock().unwrap().push((session_id, val));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_server_with_env(
    env: HashMap<String, String>,
    metadata: impl SessionMetadata + 'static,
) -> (TestServer, AppState<LocalBackend>) {
    let mut state = AppState::new_local().with_metadata(metadata);
    state.env = Arc::new(env);
    let allowed_origins = vec![
        "http://localhost".to_string(),
        "http://127.0.0.1".to_string(),
        "http://[::1]".to_string(),
    ];
    (
        TestServer::builder()
            .http_transport()
            .build(create_router(state.clone(), &allowed_origins))
            .expect("Failed starting test server"),
        state,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `on_session_created` receives the value injected into the env snapshot.
#[tokio::test]
async fn test_metadata_reads_env_key_on_create() {
    let (capture, created, _closed) = EnvCapture::new("ROUTING_TARGET");
    let env = HashMap::from([("ROUTING_TARGET".to_string(), "region-a".to_string())]);

    let (server, _state) = build_server_with_env(env, capture);

    let res = server.post("/code-mode/session/create").await;
    res.assert_status_ok();

    let session_id = res
        .json::<pctx_session_server::model::CreateSessionResponse>()
        .session_id;
    let events = created.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, session_id);
    assert_eq!(events[0].1, "region-a");
}

/// `on_session_closed` receives the same env value.
#[tokio::test]
async fn test_metadata_reads_env_key_on_close() {
    let (capture, _created, closed) = EnvCapture::new("ROUTING_TARGET");
    let env = HashMap::from([("ROUTING_TARGET".to_string(), "region-b".to_string())]);

    let (server, _state) = build_server_with_env(env, capture);

    let create_res = server.post("/code-mode/session/create").await;
    create_res.assert_status_ok();
    let session_id = create_res
        .json::<pctx_session_server::model::CreateSessionResponse>()
        .session_id;

    server
        .post("/code-mode/session/close")
        .add_header("x-code-mode-session", session_id.to_string())
        .await
        .assert_status_ok();

    let events = closed.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, session_id);
    assert_eq!(events[0].1, "region-b");
}

/// When the requested key is absent, the hook still fires with an empty string.
#[tokio::test]
async fn test_metadata_missing_key_is_empty_string() {
    let (capture, created, _closed) = EnvCapture::new("ROUTING_TARGET");

    let (server, _state) = build_server_with_env(HashMap::new(), capture);

    server
        .post("/code-mode/session/create")
        .await
        .assert_status_ok();

    let events = created.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1, "", "missing key should produce empty string");
}

/// All sessions see the same static env snapshot — it is not re-read per request.
#[tokio::test]
async fn test_metadata_env_consistent_across_sessions() {
    let (capture, created, _closed) = EnvCapture::new("ROUTING_TARGET");
    let env = HashMap::from([("ROUTING_TARGET".to_string(), "region-c".to_string())]);

    let (server, _state) = build_server_with_env(env, capture);

    for _ in 0..3 {
        server
            .post("/code-mode/session/create")
            .await
            .assert_status_ok();
    }

    let events = created.lock().unwrap();
    assert_eq!(events.len(), 3);
    for (_, val) in events.iter() {
        assert_eq!(val, "region-c");
    }
}
