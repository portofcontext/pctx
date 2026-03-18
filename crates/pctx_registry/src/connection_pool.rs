use std::{collections::HashMap, sync::Arc};

use rmcp::{RoleClient, model::InitializeRequestParams, service::RunningService};
use tokio::sync::RwLock;
use tracing::debug;

use crate::error::RegistryError;
use pctx_config::server::ServerConfig;

type PooledClient = Arc<RunningService<RoleClient, InitializeRequestParams>>;

/// A pool of cached upstream MCP connections, keyed by server name.
///
/// Connections are lazy-initialized on first use and reused for subsequent
/// calls within the same pool lifetime. The pool is cheap to clone — all
/// clones share the same underlying map.
///
/// Drop the pool (or call [`cancel_all`]) to shut down all active connections.
///
/// [`cancel_all`]: McpConnectionPool::cancel_all
#[derive(Clone, Default, Debug)]
pub struct McpConnectionPool {
    connections: Arc<RwLock<HashMap<String, PooledClient>>>,
}

impl McpConnectionPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a cached live connection for `cfg.name`, connecting fresh if
    /// none exists or if the existing connection has closed.
    ///
    /// Uses double-checked locking so connections to different servers are
    /// established concurrently. If two tasks race to connect to the same
    /// server, one wins and the other's connection is immediately cancelled.
    pub async fn get_or_connect(&self, cfg: &ServerConfig) -> Result<PooledClient, RegistryError> {
        // Fast path: return cached live connection under read lock.
        {
            let connections = self.connections.read().await;
            if let Some(client) = connections.get(&cfg.name) {
                if !client.is_closed() {
                    debug!(server = %cfg.name, "Reusing cached upstream connection");
                    return Ok(client.clone());
                }
                debug!(server = %cfg.name, "Cached connection is closed, reconnecting");
            }
        }

        // Slow path: connect outside the lock so other servers are unblocked.
        debug!(server = %cfg.name, "Connecting to upstream MCP server");
        let new_client = Arc::new(cfg.connect().await.map_err(RegistryError::from)?);

        // Write lock: upsert, preferring any live connection a concurrent task
        // may have established while we were connecting.
        let mut connections = self.connections.write().await;
        if let Some(existing) = connections.get(&cfg.name) {
            if !existing.is_closed() {
                // Another task won the race — cancel ours (Drop will handle it)
                // and return theirs.
                debug!(server = %cfg.name, "Lost connection race, using existing");
                return Ok(existing.clone());
            }
        }
        connections.insert(cfg.name.clone(), new_client.clone());
        Ok(new_client)
    }

    /// Cancels and removes all active upstream connections.
    ///
    /// Ongoing in-flight requests will complete or fail as the underlying
    /// transport shuts down. After this call the pool is empty and new calls
    /// to [`get_or_connect`] will establish fresh connections.
    ///
    /// [`get_or_connect`]: McpConnectionPool::get_or_connect
    pub async fn cancel_all(&self) {
        let mut connections = self.connections.write().await;
        let count = connections.len();
        for (name, client) in connections.drain() {
            debug!(server = %name, "Cancelling upstream connection");
            client.cancellation_token().cancel();
            // Arc drop here triggers RunningService::Drop which also cancels,
            // but the explicit cancel above ensures immediate signalling even
            // if other Arc clones are still held by in-flight calls.
        }
        if count > 0 {
            debug!(count, "Cancelled all upstream connections");
        }
    }

    /// Returns the number of cached connections currently in the pool.
    pub async fn len(&self) -> usize {
        self.connections.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.connections.read().await.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pool_starts_empty() {
        let pool = McpConnectionPool::new();
        assert!(pool.is_empty().await);
    }

    #[tokio::test]
    async fn cancel_all_on_empty_pool_is_a_noop() {
        let pool = McpConnectionPool::new();
        pool.cancel_all().await; // must not panic
        assert!(pool.is_empty().await);
    }
}
