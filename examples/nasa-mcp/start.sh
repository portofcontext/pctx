#!/bin/bash
set -e

echo "Starting NASA MCP server on port ${NASA_MCP_PORT}..."
node /app/nasa-mcp-server.js &
NASA_PID=$!

echo "Waiting for NASA MCP server to start..."
# Wait for the server to start by checking if the port is open
for i in {1..30}; do
  if curl -s -X POST http://127.0.0.1:${NASA_MCP_PORT}/mcp \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}},"id":1}' > /dev/null 2>&1; then
    echo "NASA MCP server started successfully"
    break
  fi

  if [ $i -eq 30 ]; then
    echo "ERROR: NASA MCP server failed to start after 30 seconds"
    exit 1
  fi
  sleep 1
done

echo "Starting pctx on port ${PCTX_PORT}..."
exec pctx --config /app/pctx.json code-mode start --port ${PCTX_PORT} --host 0.0.0.0
