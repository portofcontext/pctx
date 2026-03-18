use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use pctx_code_mode::{
    CodeMode,
    model::{ExecuteTypescriptInput, ExecuteTypescriptOutput},
    registry::McpConnectionPool,
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[async_trait]
pub trait PctxSessionBackend: Clone + Send + Sync + 'static {
    /// Retrieve a `CodeMode` struct by it's session ID from the backend
    async fn get(&self, session_id: Uuid) -> Result<Option<CodeMode>>;

    /// Add a new `CodeMode` struct to the backend
    async fn insert(&self, session_id: Uuid, code_mode: CodeMode) -> Result<()>;

    /// Update a `CodeMode` struct as a full replacement (PUT not PATCH)
    /// in the backend
    async fn update(&self, session_id: Uuid, code_mode: CodeMode) -> Result<()>;

    /// Deletes a `CodeMode` struct from the backend, returning the deleted
    /// instance if it exists.
    async fn delete(&self, session_id: Uuid) -> Result<bool>;

    /// Checks if a `CodeMode` struct exists for the given ID
    async fn exists(&self, session_id: Uuid) -> Result<bool>;

    /// Returns the number of active `CodeMode` sessions in the backend
    async fn count(&self) -> Result<usize>;

    /// Returns a full list of active `CodeMode` sessions in the backend.
    async fn list_sessions(&self) -> Result<Vec<Uuid>>;

    /// Returns the cached MCP connection pool for a session, if any.
    async fn get_pool(&self, _session_id: Uuid) -> Result<Option<Arc<McpConnectionPool>>> {
        Ok(None)
    }

    /// Caches an MCP connection pool for a session.
    async fn set_pool(&self, _session_id: Uuid, _pool: Arc<McpConnectionPool>) -> Result<()> {
        Ok(())
    }

    /// Hook called after every code mode execution websocket event
    async fn post_execution(
        &self,
        _session_id: Uuid,
        _execution_id: Uuid,
        _code_mode: CodeMode,
        _execution_req: ExecuteTypescriptInput,
        _execution_res: Result<ExecuteTypescriptOutput>,
    ) -> Result<()> {
        Ok(())
    }
}

/// Manages `CodeMode` sessions locally using thread-safe
/// smart references and read/write locks
#[derive(Clone, Default)]
pub struct LocalBackend {
    /// Map of `session_id` -> `Arc<RwLock<CodeMode>>`
    /// Each `CodeMode` has its own lock for better concurrency
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<CodeMode>>>>>,
    /// Cached MCP connection pools keyed by session ID. Allows stateful
    /// upstream MCP connections (e.g. LSP) to survive across executions
    /// within the same session.
    pools: Arc<RwLock<HashMap<Uuid, Arc<McpConnectionPool>>>>,
}

#[async_trait]
impl PctxSessionBackend for LocalBackend {
    async fn get(&self, session_id: Uuid) -> Result<Option<CodeMode>> {
        let sessions = self.sessions.read().await;
        match sessions.get(&session_id) {
            Some(code_mode_lock) => Ok(Some(code_mode_lock.read().await.clone())),
            None => Ok(None),
        }
    }

    async fn insert(&self, session_id: Uuid, code_mode: CodeMode) -> Result<()> {
        let code_mode_lock = Arc::new(RwLock::new(code_mode));
        self.sessions
            .write()
            .await
            .insert(session_id, code_mode_lock);

        Ok(())
    }

    async fn update(&self, session_id: Uuid, code_mode: CodeMode) -> Result<()> {
        let sessions = self.sessions.read().await;
        let to_update = sessions
            .get(&session_id)
            .context(format!("CodeMode session {session_id} does not exist"))?;

        *to_update.write().await = code_mode;

        Ok(())
    }

    async fn delete(&self, session_id: Uuid) -> Result<bool> {
        let deleted = self.sessions.write().await.remove(&session_id);
        if deleted.is_some() {
            if let Some(pool) = self.pools.write().await.remove(&session_id) {
                pool.cancel_all().await;
            }
        }
        Ok(deleted.is_some())
    }

    async fn exists(&self, session_id: Uuid) -> Result<bool> {
        Ok(self.sessions.read().await.contains_key(&session_id))
    }

    async fn count(&self) -> Result<usize> {
        Ok(self.sessions.read().await.len())
    }

    async fn list_sessions(&self) -> Result<Vec<Uuid>> {
        Ok(self.sessions.read().await.keys().copied().collect())
    }

    async fn get_pool(&self, session_id: Uuid) -> Result<Option<Arc<McpConnectionPool>>> {
        Ok(self.pools.read().await.get(&session_id).cloned())
    }

    async fn set_pool(&self, session_id: Uuid, pool: Arc<McpConnectionPool>) -> Result<()> {
        self.pools.write().await.insert(session_id, pool);
        Ok(())
    }
}
