#!/bin/bash
set -e

# CLI Integration Test Script for pctx mcp start
# Tests the full CLI with both stdio and HTTP MCP servers

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Create temp directory for test files
TEST_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Cleanup function
cleanup() {
    echo -e "${YELLOW}Cleaning up...${NC}"
    if [ -f "$TEST_DIR/pctx-test.pid" ]; then
        kill $(cat "$TEST_DIR/pctx-test.pid") 2>/dev/null || true
    fi
    # Kill any processes on port 8080 (may not exist, so ignore errors)
    lsof -ti:8080 2>/dev/null | xargs kill -9 2>/dev/null || true
    rm -rf "$TEST_DIR"
}

trap cleanup EXIT

echo -e "${GREEN}Starting CLI Integration Tests${NC}"
echo "======================================"

# Test 1: Start server (tests HTTP endpoint with parallel MCP initialization)
echo -e "\n${YELLOW}Test 1: Starting pctx MCP server${NC}"

# Create test config with multiple stdio MCP servers
# This tests parallel initialization of multiple stdio servers
cat > "$TEST_DIR/pctx-test.json" <<'EOF'
{
  "name": "pctx-test",
  "version": "0.1.0",
  "servers": [
    {
      "name": "memory",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-memory"]
    },
    {
      "name": "filesystem",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    }
  ]
}
EOF

# Use PCTX_CMD if set (for CI with pre-built binary), otherwise use cargo run
PCTX_CMD="${PCTX_CMD:-cargo run --bin pctx --}"

cd "$PROJECT_ROOT"
$PCTX_CMD mcp start \
    --config "$TEST_DIR/pctx-test.json" \
    --no-banner \
    > "$TEST_DIR/pctx-test.log" 2>&1 &
echo $! > "$TEST_DIR/pctx-test.pid"

# Wait for server to be ready (check logs for initialization message)
echo "Waiting for server to start..."
for i in {1..60}; do
    # Check if server has initialized by looking for log message
    if grep -q "PCTX listening at" "$TEST_DIR/pctx-test.log" 2>/dev/null; then
        echo -e "${GREEN}✓ Server started successfully in $i seconds${NC}"
        break
    fi
    if [ $i -eq 60 ]; then
        echo -e "${RED}✗ Server failed to start within 60 seconds${NC}"
        echo "Server logs:"
        cat "$TEST_DIR/pctx-test.log"
        exit 1
    fi
    sleep 1
done

# Give server a moment to be fully ready
sleep 1

# Test 2: MCP endpoint check
echo -e "\n${YELLOW}Test 2: MCP endpoint check${NC}"
mcp_response=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}')

if echo "$mcp_response" | grep -q "result"; then
    echo -e "${GREEN}✓ MCP endpoint is responding${NC}"
else
    echo -e "${RED}✗ MCP endpoint check failed${NC}"
    echo "Response: $mcp_response"
    echo "Server logs:"
    tail -30 "$TEST_DIR/pctx-test.log" | grep -v 'code="async function' | tail -10
    exit 1
fi

# Test 3: List tools via MCP protocol
echo -e "\n${YELLOW}Test 3: List tools from MCP server${NC}"
response=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d '{
      "jsonrpc": "2.0",
      "id": 1,
      "method": "tools/list"
    }')

echo "Response preview: ${response:0:200}..."

# Check that we got tools back
if echo "$response" | grep -q '"tools"'; then
    echo -e "${GREEN}✓ Successfully listed tools from MCP server${NC}"

    # Count how many tools we found
    tool_count=$(echo "$response" | grep -o '"name"' | wc -l | tr -d ' ')
    echo "  Found $tool_count tools"
else
    echo -e "${RED}✗ No tools found in response${NC}"
    echo "Full response: $response"
    exit 1
fi

# Test 4: Call list_functions tool
echo -e "\n${YELLOW}Test 4: Call list_functions${NC}"
list_response=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d '{
      "jsonrpc": "2.0",
      "id": 2,
      "method": "tools/call",
      "params": {
        "name": "list_functions",
        "arguments": {}
      }
    }')

if echo "$list_response" | grep -q '"result"'; then
    echo -e "${GREEN}✓ list_functions called successfully${NC}"
    # Check that we got function definitions back
    if echo "$list_response" | grep -q '"functions"'; then
        echo "  Response contains function definitions"
        # Extract and count function namespaces
        namespace_count=$(echo "$list_response" | grep -o '"[A-Z][a-zA-Z]*\.' | sort -u | wc -l | tr -d ' ')
        echo "  Found $namespace_count namespaces"
    fi
else
    echo -e "${RED}✗ list_functions call failed${NC}"
    echo "Response: $list_response"
    exit 1
fi

# Test 5: Call get_function_details tool
echo -e "\n${YELLOW}Test 5: Call get_function_details${NC}"
details_response=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d '{
      "jsonrpc": "2.0",
      "id": 3,
      "method": "tools/call",
      "params": {
        "name": "get_function_details",
        "arguments": {
          "functions": ["Memory.createEntities"]
        }
      }
    }')

if echo "$details_response" | grep -q '"result"'; then
    echo -e "${GREEN}✓ get_function_details called successfully${NC}"
    # Check that we got function details back
    if echo "$details_response" | grep -q '"functions"'; then
        echo "  Response contains function details"
    fi
else
    echo -e "${RED}✗ get_function_details call failed${NC}"
    echo "Response: $details_response"
    exit 1
fi

# Test 6: Call execute tool
echo -e "\n${YELLOW}Test 6: Call execute with TypeScript code${NC}"
execute_response=$(curl -s -X POST http://localhost:8080/mcp \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d "{
      \"jsonrpc\": \"2.0\",
      \"id\": 4,
      \"method\": \"tools/call\",
      \"params\": {
        \"name\": \"execute\",
        \"arguments\": {
          \"code\": \"async function run() { const result = await Memory.createEntities({ entities: [{ name: 'test', entityType: 'item', observations: ['test observation'] }] }); return result; }\"
        }
      }
    }")

if echo "$execute_response" | grep -q '"result"'; then
    echo -e "${GREEN}✓ execute called successfully${NC}"
    # Check that the code execution succeeded (not just that we got a response)
    if echo "$execute_response" | grep -q '"success":true'; then
        echo "  Code executed successfully and returned result"
    else
        echo -e "${RED}✗ Code execution failed${NC}"
        echo "Response: $execute_response"
        exit 1
    fi
else
    echo -e "${RED}✗ execute call failed${NC}"
    echo "Response: $execute_response"
    exit 1
fi

# Test 7: Verify MCP server initialization in logs
echo -e "\n${YELLOW}Test 7: Verify MCP server initialization${NC}"
echo "Checking server logs..."

if grep -q "Creating code mode interface" "$TEST_DIR/pctx-test.log"; then
    echo -e "${GREEN}✓ MCP server initialization logged${NC}"

    # Check if any servers were initialized
    if grep -q "Successfully initialized MCP server\|PCTX listening" "$TEST_DIR/pctx-test.log"; then
        echo -e "${GREEN}✓ Server started successfully${NC}"
    fi
else
    echo -e "${YELLOW}⚠ MCP server initialization logs not found${NC}"
fi

# Show summary
echo -e "\n${GREEN}======================================"
echo "✓ All tests PASSED!"
echo -e "======================================${NC}"
echo ""

if [ "${SHOW_LOGS}" = "1" ]; then
    echo -e "${YELLOW}Server logs:${NC}"
    echo "---"
    tail -50 "$TEST_DIR/pctx-test.log" | grep -v 'code="async function' | tail -10
    echo "---"
    echo ""
fi

exit 0
