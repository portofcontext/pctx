#!/bin/bash
# Start both HTTP and stdio MCP test servers for integration testing
#
# Usage: ./start_test_mcp_servers.sh
#
# This will start the HTTP MCP server in the background.
# The stdio MCP server is spawned automatically by tests as needed.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Starting HTTP MCP test server on http://localhost:8765/mcp ..."
python "${SCRIPT_DIR}/test_http_mcp_server.py" &
HTTP_PID=$!

echo "HTTP MCP server started with PID: $HTTP_PID"
echo ""
echo "Test servers are ready!"
echo "- HTTP MCP server: http://localhost:8765/mcp"
echo "- Stdio MCP server: Auto-spawned by tests"
echo ""
echo "Run integration tests with: pytest --integration"
echo ""
echo "Press Ctrl+C to stop the HTTP MCP server"

# Wait for HTTP server and cleanup on exit
trap "kill $HTTP_PID 2>/dev/null" EXIT
wait $HTTP_PID
