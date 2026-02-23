use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use tracing::instrument;

/// An async callback function that can be registered and invoked from sandboxed code.
///
/// Both the JavaScript (`pctx_code_execution_runtime`) and Python (`pctx_python_runtime`)
/// runtimes share this type so the same closures can be registered with either.
pub type CallbackFn = Arc<
    dyn Fn(
            Option<serde_json::Value>,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
        + Send
        + Sync,
>;

/// Registry mapping callback names to their implementations.
///
/// Clone is cheap — the inner map is reference-counted.
#[derive(Clone, Default)]
pub struct CallbackRegistry {
    callbacks: Arc<RwLock<HashMap<String, CallbackFn>>>,
}

impl std::fmt::Debug for CallbackRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackRegistry")
            .field("ids", &self.ids())
            .finish()
    }
}

impl CallbackRegistry {
    /// Returns the ids registered in this [`CallbackRegistry`].
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn ids(&self) -> Vec<String> {
        self.callbacks
            .read()
            .unwrap()
            .keys()
            .map(String::from)
            .collect()
    }

    /// Register a callback under `id`.
    ///
    /// # Errors
    ///
    /// Returns an error if a callback with the same `id` is already registered.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn add(&self, id: &str, callback: CallbackFn) -> Result<(), String> {
        let mut callbacks = self
            .callbacks
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {e}"))?;

        if callbacks.contains_key(id) {
            return Err(format!("Callback \"{id}\" is already registered"));
        }

        callbacks.insert(id.to_owned(), callback);
        Ok(())
    }

    /// Remove a callback from the registry.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn remove(&self, id: &str) -> Option<CallbackFn> {
        self.callbacks.write().unwrap().remove(id)
    }

    /// Look up a callback by id.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn get(&self, id: &str) -> Option<CallbackFn> {
        self.callbacks.read().unwrap().get(id).cloned()
    }

    /// Returns `true` if a callback with the given `id` is registered.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn has(&self, id: &str) -> bool {
        self.callbacks.read().unwrap().contains_key(id)
    }

    /// Invoke a callback by id with the given JSON arguments.
    ///
    /// # Errors
    ///
    /// Returns an error string if the id is not registered or if the callback fails.
    #[instrument(name = "invoke_callback", skip_all, fields(id = id))]
    pub async fn invoke(
        &self,
        id: &str,
        args: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let callback = self
            .get(id)
            .ok_or_else(|| format!("Callback \"{id}\" is not registered"))?;

        callback(args).await
    }
}
