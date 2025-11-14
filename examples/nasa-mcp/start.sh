#!/bin/bash
set -e

# Create a temporary file to capture the auth token
NASA_LOG=$(mktemp)

echo "Starting NASA MCP server on port ${NASA_MCP_PORT}..."
node /app/nasa-mcp-server.js > "$NASA_LOG" 2>&1 &
NASA_PID=$!

echo "Waiting for NASA MCP server to start and capturing auth token..."
NASA_AUTH_TOKEN=""
for i in {1..30}; do
  # Check if the auth token has been generated
  if grep -q "Generated auth token:" "$NASA_LOG"; then
    NASA_AUTH_TOKEN=$(grep "Generated auth token:" "$NASA_LOG" | sed 's/.*Generated auth token: //')
    echo "Captured auth token from NASA MCP server"
    break
  fi

  if [ $i -eq 30 ]; then
    echo "Failed to capture auth token from NASA MCP server"
    cat "$NASA_LOG"
    rm "$NASA_LOG"
    exit 1
  fi
  sleep 1
done

# Wait a bit more for the server to fully start
sleep 2

# Verify server is responding
if ! curl -s -f -H "Authorization: Bearer ${NASA_AUTH_TOKEN}" http://127.0.0.1:${NASA_MCP_PORT}/mcp > /dev/null 2>&1; then
  echo "NASA MCP server is not responding"
  cat "$NASA_LOG"
  rm "$NASA_LOG"
  exit 1
fi

echo "NASA MCP server is ready!"

# Export the auth token for pctx to use
export NASA_MCP_AUTH_TOKEN="${NASA_AUTH_TOKEN}"

echo "Starting pctx on port ${PCTX_PORT}..."
pctx --config /app/pctx.json start --port ${PCTX_PORT} --host 0.0.0.0 > >(tee -a "$NASA_LOG") 2>&1 &
PCTX_PID=$!

# Keep tailing the log
tail -f "$NASA_LOG" &
TAIL_PID=$!

# Function to handle shutdown
shutdown() {
  echo "Shutting down services..."
  kill $TAIL_PID $NASA_PID $PCTX_PID 2>/dev/null || true
  wait $NASA_PID $PCTX_PID 2>/dev/null || true
  rm -f "$NASA_LOG"
  exit 0
}

trap shutdown SIGTERM SIGINT

# Wait for either process to exit
wait -n $NASA_PID $PCTX_PID
EXIT_CODE=$?

# If either process exits, shut down the other
shutdown
