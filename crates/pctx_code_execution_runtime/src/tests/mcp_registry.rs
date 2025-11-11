//! Unit tests for MCP Registry operations

use pctx_config::server::ServerConfig;

use crate::registry::MCPRegistry;

#[test]
fn test_registry_new() {
    let registry = MCPRegistry::new();
    assert!(!registry.has("test-server"), "New registry should be empty");
}

#[test]
fn test_registry_add_and_has() {
    let registry = MCPRegistry::new();

    let config = ServerConfig::new(
        "test-server".into(),
        "http://localhost:3000".parse().unwrap(),
    );

    registry
        .add(config)
        .expect("Should add server successfully");
    assert!(registry.has("test-server"), "Server should be registered");
}

#[test]
fn test_registry_add_duplicate_fails() {
    let registry = MCPRegistry::new();

    let config1 = ServerConfig::new(
        "duplicate-server".into(),
        "http://localhost:3000".parse().unwrap(),
    );

    let config2 = ServerConfig::new(
        "duplicate-server".into(),
        "http://localhost:3001".parse().unwrap(),
    );

    registry.add(config1).expect("First add should succeed");

    let result = registry.add(config2);
    assert!(result.is_err(), "Duplicate registration should fail");

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("already registered"),
        "Error message should mention duplicate registration, got: {err}"
    );
}

#[test]
fn test_registry_get() {
    let registry = MCPRegistry::new();

    let config = ServerConfig::new("my-server".into(), "http://localhost:4000".parse().unwrap());

    registry.add(config.clone()).expect("Should add server");

    let retrieved = registry.get("my-server").expect("Should retrieve server");
    assert_eq!(retrieved.name, "my-server");
    assert_eq!(retrieved.url, "http://localhost:4000".parse().unwrap());
}

#[test]
fn test_registry_get_nonexistent() {
    let registry = MCPRegistry::new();

    let result = registry.get("nonexistent-server");
    assert!(
        result.is_none(),
        "Should return None for nonexistent server"
    );
}

#[test]
fn test_registry_delete() {
    let registry = MCPRegistry::new();

    let config = ServerConfig::new(
        "temp-server".into(),
        "http://localhost:5000".parse().unwrap(),
    );

    registry.add(config).expect("Should add server");
    assert!(registry.has("temp-server"), "Server should exist");

    let deleted = registry.delete("temp-server");
    assert!(deleted, "Delete should return true");
    assert!(
        !registry.has("temp-server"),
        "Server should no longer exist"
    );
}

#[test]
fn test_registry_delete_nonexistent() {
    let registry = MCPRegistry::new();

    let deleted = registry.delete("nonexistent-server");
    assert!(
        !deleted,
        "Delete should return false for nonexistent server"
    );
}

#[test]
fn test_registry_clear() {
    let registry = MCPRegistry::new();

    let configs = vec![
        ServerConfig::new("server1".into(), "http://localhost:3000".parse().unwrap()),
        ServerConfig::new("server2".into(), "http://localhost:3001".parse().unwrap()),
        ServerConfig::new("server3".into(), "http://localhost:3002".parse().unwrap()),
    ];

    for config in configs {
        registry.add(config).expect("Should add server");
    }

    assert!(registry.has("server1"), "Server 1 should exist");
    assert!(registry.has("server2"), "Server 2 should exist");
    assert!(registry.has("server3"), "Server 3 should exist");

    registry.clear();

    assert!(!registry.has("server1"), "Server 1 should be cleared");
    assert!(!registry.has("server2"), "Server 2 should be cleared");
    assert!(!registry.has("server3"), "Server 3 should be cleared");
}

#[test]
fn test_registry_multiple_servers() {
    let registry = MCPRegistry::new();

    let servers = vec![
        ("server1", "http://localhost:3000"),
        ("server2", "http://localhost:3001"),
        ("server3", "http://localhost:3002"),
        ("server4", "http://localhost:3003"),
    ];

    for (name, url) in &servers {
        let config = ServerConfig::new((*name).into(), (*url).parse().unwrap());
        registry.add(config).expect("Should add server");
    }

    for (name, url) in &servers {
        assert!(registry.has(name), "Server {name} should exist");
        let config = registry.get(name).expect("Should get server");
        assert_eq!(
            config.url,
            url.parse().unwrap(),
            "URL should match for {name}"
        );
    }
}

#[test]
fn test_registry_clone() {
    let registry1 = MCPRegistry::new();

    let config = ServerConfig::new(
        "test-server".into(),
        "http://localhost:3000".parse().unwrap(),
    );

    registry1.add(config).expect("Should add server");

    // Clone the registry
    let registry2 = registry1.clone();

    // Both registries should share the same underlying data
    assert!(
        registry2.has("test-server"),
        "Cloned registry should have server"
    );

    // Add to registry2
    let config2 = ServerConfig::new("server2".into(), "http://localhost:3001".parse().unwrap());
    registry2
        .add(config2)
        .expect("Should add to cloned registry");

    // registry1 should also see the new server (shared state)
    assert!(
        registry1.has("server2"),
        "Original registry should see new server from clone"
    );
}
