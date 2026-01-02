"""Tests for server configuration types"""

from pctx_client.models import HttpServerConfig, StdioServerConfig


def test_http_server_config():
    """Test HTTP server configuration"""
    config: HttpServerConfig = {
        "name": "test-http",
        "url": "http://localhost:8080/mcp",
    }
    assert config["name"] == "test-http"
    assert config["url"] == "http://localhost:8080/mcp"


def test_http_server_config_with_bearer_auth():
    """Test HTTP server configuration with bearer authentication"""
    config: HttpServerConfig = {
        "name": "test-http",
        "url": "http://localhost:8080/mcp",
        "auth": {
            "type": "bearer",
            "token": "my-secret-token",
        },
    }
    assert config["name"] == "test-http"
    assert config["url"] == "http://localhost:8080/mcp"
    assert config["auth"]["type"] == "bearer"
    assert config["auth"]["token"] == "my-secret-token"


def test_http_server_config_with_headers_auth():
    """Test HTTP server configuration with headers authentication"""
    config: HttpServerConfig = {
        "name": "test-http",
        "url": "http://localhost:8080/mcp",
        "auth": {
            "type": "headers",
            "headers": {
                "X-API-Key": "my-api-key",
                "X-Custom-Header": "custom-value",
            },
        },
    }
    assert config["name"] == "test-http"
    assert config["url"] == "http://localhost:8080/mcp"
    assert config["auth"]["type"] == "headers"
    assert config["auth"]["headers"]["X-API-Key"] == "my-api-key"


def test_stdio_server_config():
    """Test stdio server configuration"""
    config: StdioServerConfig = {
        "name": "test-stdio",
        "command": "node",
    }
    assert config["name"] == "test-stdio"
    assert config["command"] == "node"


def test_stdio_server_config_with_args():
    """Test stdio server configuration with arguments"""
    config: StdioServerConfig = {
        "name": "test-stdio",
        "command": "node",
        "args": ["./server.js", "--port", "3000"],
    }
    assert config["name"] == "test-stdio"
    assert config["command"] == "node"
    assert config["args"] == ["./server.js", "--port", "3000"]


def test_stdio_server_config_with_env():
    """Test stdio server configuration with environment variables"""
    config: StdioServerConfig = {
        "name": "test-stdio",
        "command": "node",
        "args": ["./server.js"],
        "env": {
            "NODE_ENV": "production",
            "API_KEY": "secret",
        },
    }
    assert config["name"] == "test-stdio"
    assert config["command"] == "node"
    assert config["args"] == ["./server.js"]
    assert config["env"]["NODE_ENV"] == "production"
    assert config["env"]["API_KEY"] == "secret"
