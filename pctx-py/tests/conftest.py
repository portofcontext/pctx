"""Pytest configuration for pctx test suite"""

import os
import subprocess
import time
import pytest


def pytest_addoption(parser):
    """Add custom command line options"""
    parser.addoption(
        "--integration",
        action="store_true",
        default=False,
        help="Run integration tests that require a running pctx server",
    )


def pytest_configure(config):
    """Register custom markers"""
    config.addinivalue_line(
        "markers", "integration: mark test as integration test (requires --integration flag)"
    )


def pytest_collection_modifyitems(config, items):
    """Skip integration tests unless --integration flag is provided"""
    if config.getoption("--integration"):
        # Running with --integration flag, run all tests
        return

    # Skip integration tests by default
    skip_integration = pytest.mark.skip(reason="need --integration flag to run")
    for item in items:
        if "integration" in item.keywords:
            item.add_marker(skip_integration)


@pytest.fixture(scope="session")
def http_mcp_server():
    """Start HTTP MCP test server for the test session"""
    import socket

    # Get path to HTTP MCP server script
    script_path = os.path.join(
        os.path.dirname(__file__), "scripts", "test_http_mcp_server.py"
    )

    # Start the HTTP MCP server in background
    process = subprocess.Popen(
        ["python", script_path],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    # Wait for server to be ready by checking if port is listening
    max_attempts = 20  # 10 seconds total
    for attempt in range(max_attempts):
        time.sleep(0.5)

        # Check if process crashed
        if process.poll() is not None:
            raise RuntimeError(
                f"HTTP MCP server process exited with code {process.returncode}"
            )

        # Check if port is listening
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            result = sock.connect_ex(('127.0.0.1', 8765))
            sock.close()
            if result == 0:
                # Port is open, server is ready
                break
        except:
            pass
    else:
        # Cleanup failed server
        process.terminate()
        process.wait()
        raise RuntimeError(
            f"HTTP MCP server failed to start listening on port 8765 after {max_attempts * 0.5}s"
        )

    yield process

    # Cleanup: terminate the server
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
