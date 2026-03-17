# Upstream MCP Servers

Connect multiple MCP servers through a single interface with unified authentication.

## Overview

PCTX aggregates multiple MCP servers into a single endpoint, allowing AI agents to interact with many services through one interface.

**Key Points:**

- **MCP Server to AI Agents**: PCTX exposes a single MCP server interface
- **MCP Client to Upstream**: PCTX acts as an MCP client to multiple upstream servers
- **Bring Your Own LLM**: Works with any AI agent (Claude, ChatGPT, custom agents)
- **Deploy Anywhere**: Run locally or in the cloud with full control

### Single Interface

```
AI Agent
    ↓
PCTX (localhost:8080)
    ├→ Google Drive MCP
    ├→ Slack MCP
    ├→ GitHub MCP
    └→ Custom Internal MCP
```

Instead of configuring each MCP server separately in your AI tool, configure PCTX once.

## How It Works

### 1. Server Registration

Each server is registered with a unique name.

The name becomes the TypeScript namespace for that server's tools.

### 2. Tool Aggregation

When PCTX starts, it:

1. Connects to each configured server
2. Fetches tool definitions from each
3. Generates TypeScript namespaces
4. Exposes all tools through a single endpoint

### 3. Namespace Organization

Each server's tools are accessible via its namespace:

```typescript
// Google Drive tools
await gdrive.getSheet({ sheetId: "abc" });
await gdrive.createDocument({ title: "Report" });

// Slack tools
await slack.sendMessage({ channel: "#general", text: "hi" });
await slack.getUsers();

// Internal tools
await internal.processOrder({ orderId: "123" });
await internal.sendNotification({ type: "email" });
```

## Authentication Management

PCTX handles authentication separately for each server. See [Configuration docs](config.md) for details.

PCTX acts as a proxy, forwarding tool calls to the appropriate upstream server based on namespace.

## Upstream Sessions

Some upstream MCP servers are stateful — they maintain internal state across multiple tool calls (e.g. an LSP server that tracks open files, or a database connection that holds a transaction). PCTX preserves upstream connections across `execute_typescript` calls using a connection pool, so the upstream server sees a continuous session rather than a series of disconnected requests.

How the pool is scoped depends on which PCTX command you use.

### `pctx start` (session server)

The session server is used by the Python SDK. Upstream connections are scoped to a **code mode session**.

- When a session runs its first `execute_typescript`, PCTX creates a connection pool and connects to all configured upstream servers.
- Subsequent executions within the same session reuse those connections — the upstream MCP sees an uninterrupted session.
- When the session is deleted, all upstream connections are shut down cleanly.

Each active session has its own isolated pool; two concurrent sessions never share upstream connections.

### `pctx mcp start` (unified MCP server)

The unified MCP server is what AI clients like Claude Desktop connect to directly. It supports three modes with different session scoping:

#### HTTP (default — stateless)

```bash
pctx mcp start
```

Each request gets a fresh connection pool. Upstream connections are dropped after the request completes. Use this when your upstream MCPs are stateless or when you don't need to preserve state across code mode tool calls.

#### HTTP with stateful sessions (`--stateful-http`)

```bash
pctx mcp start --stateful-http
```

Upstream connections are scoped to an **HTTP session**, identified by the `mcp-session-id` header that the MCP client sends. This is part of the [MCP streamable HTTP spec](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports#streamable-http).

- On the first request for a session ID, a connection pool is created and cached.
- Subsequent requests from the same client reuse the cached pool.
- When the client ends the session (HTTP `DELETE` to the session endpoint), the pool is cancelled and all upstream connections are closed.

Use this mode when connecting to stateful upstream MCPs over HTTP and your client supports MCP session management.

#### Stdio (`--stdio`)

```bash
pctx mcp start --stdio
```

When running as a stdio MCP server (e.g. configured in Claude Desktop's `mcpServers`), the entire process lifetime is treated as a single session. A global session ID is assigned at startup, and all `execute_typescript` calls share one connection pool for the life of the process.

This means upstream MCP servers (like an LSP) connect once when first used and stay connected until `pctx` exits.
