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
await gdrive.getSheet({ sheetId: 'abc' });
await gdrive.createDocument({ title: 'Report' });

// Slack tools
await slack.sendMessage({ channel: '#general', text: 'hi' });
await slack.getUsers();

// Internal tools
await internal.processOrder({ orderId: '123' });
await internal.sendNotification({ type: 'email' });
```

## Authentication Management

PCTX handles authentication separately for each server. See [MCP Authentication Guide](mcp-auth.md) for details.

PCTX acts as a proxy, forwarding tool calls to the appropriate upstream server based on namespace.


## Learn More

- [Code Mode Interface](code-mode.md) - How tools are exposed as TypeScript functions
- [MCP Authentication](mcp-auth.md) - Configuring authentication for each server
- [Model Context Protocol](https://modelcontextprotocol.io/) - MCP specification
