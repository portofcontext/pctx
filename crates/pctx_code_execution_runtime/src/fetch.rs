//! Fetch implementation with host-based permissions
//!
//! This module provides a fetch function that only allows requests to specific allowed hosts

use crate::error::McpError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// Allowed hosts registry for network permissions
#[derive(Debug, Clone)]
pub struct AllowedHosts {
    hosts: Arc<RwLock<HashSet<String>>>,
}

impl AllowedHosts {
    pub fn new(hosts: Option<Vec<String>>) -> Self {
        let host_set = hosts
            .unwrap_or_default()
            .into_iter()
            .collect::<HashSet<String>>();

        Self {
            hosts: Arc::new(RwLock::new(host_set)),
        }
    }

    /// Check if a host is allowed for network access
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn is_allowed(&self, host: &str) -> bool {
        let hosts = self.hosts.read().expect("AllowedHosts lock poisoned");
        // If no hosts are configured, block all requests
        if hosts.is_empty() {
            return false;
        }
        hosts.contains(host)
    }

    /// Add a host to the allowed list
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn add(&self, host: String) {
        let mut hosts = self.hosts.write().expect("AllowedHosts lock poisoned");
        hosts.insert(host);
    }

    /// Remove a host from the allowed list
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn remove(&self, host: &str) -> bool {
        let mut hosts = self.hosts.write().expect("AllowedHosts lock poisoned");
        hosts.remove(host)
    }

    /// Clear all hosts from the allowed list
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (i.e., a thread panicked while holding the lock)
    pub fn clear(&self) {
        let mut hosts = self.hosts.write().expect("AllowedHosts lock poisoned");
        hosts.clear();
    }
}

impl Default for AllowedHosts {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Fetch request options
#[derive(Debug, Deserialize)]
pub(crate) struct FetchOptions {
    pub method: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub body: Option<String>,
}

/// Fetch response
#[derive(Debug, Serialize)]
pub(crate) struct FetchResponse {
    pub status: u16,
    pub headers: serde_json::Value,
    pub body: String,
}

/// Perform a fetch request with host permissions
pub(crate) async fn fetch_with_permissions(
    url: String,
    options: Option<FetchOptions>,
    allowed_hosts: &AllowedHosts,
) -> Result<FetchResponse, McpError> {
    // Parse URL and extract host (with port if present)
    let parsed_url =
        url::Url::parse(&url).map_err(|e| McpError::ToolCallError(format!("Invalid URL: {e}")))?;

    let host_str = parsed_url
        .host_str()
        .ok_or_else(|| McpError::ToolCallError("URL has no host".to_string()))?;

    // Build host:port string for permission checking
    let host_with_port = if let Some(port) = parsed_url.port() {
        format!("{host_str}:{port}")
    } else {
        host_str.to_string()
    };

    // Check permissions (try both with and without port)
    if !allowed_hosts.is_allowed(&host_with_port) && !allowed_hosts.is_allowed(host_str) {
        return Err(McpError::ToolCallError(format!(
            "Network access to host '{host_with_port}' is not allowed"
        )));
    }

    // Build request
    let client = reqwest::Client::new();
    let method = options
        .as_ref()
        .and_then(|o| o.method.as_deref())
        .unwrap_or("GET");

    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => {
            return Err(McpError::ToolCallError(format!(
                "Unsupported HTTP method: {method}"
            )));
        }
    };

    // Add headers if provided
    if let Some(ref opts) = options {
        if let Some(headers_val) = &opts.headers
            && let Some(headers_obj) = headers_val.as_object()
        {
            for (key, value) in headers_obj {
                if let Some(value_str) = value.as_str() {
                    request = request.header(key, value_str);
                }
            }
        }

        // Add body if provided
        if let Some(ref body) = opts.body {
            request = request.body(body.clone());
        }
    }

    // Execute request
    let response = request
        .send()
        .await
        .map_err(|e| McpError::ToolCallError(format!("Fetch failed: {e}")))?;

    let status = response.status().as_u16();

    // Extract headers
    let headers_map: serde_json::Map<String, serde_json::Value> = response
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                serde_json::Value::String(v.to_str().unwrap_or("").to_string()),
            )
        })
        .collect();

    let body = response
        .text()
        .await
        .map_err(|e| McpError::ToolCallError(format!("Failed to read response body: {e}")))?;

    Ok(FetchResponse {
        status,
        headers: serde_json::Value::Object(headers_map),
        body,
    })
}
