todo: centered large logo

# PCTX
---
todo: banners

# Getting Started

pctx enables you to create connected AI agents by aggregating multiple MCP servers, exposing tools as TypeScript functions for efficient code-mode execution.

Visit our (Learn pctx)[todo] course to get started with pctx.
Visit the (pctx Showcase)[todo] to see ai agents build with pctx.

# Documentation

Visit https://portofcontext.com/ptcx/docs to view the full documentation.



## What is Code Mode?

Unlike traditional MCP implementations where agents directly call tools, PCTX exposes MCP tools as TypeScript functions. This allows AI agents to write code that calls MCP servers more efficiently by:

- **Loading tools on-demand**: Only load the tool definitions needed for the current task, rather than all tools upfront
- **Processing data efficiently**: Filter and transform data in the execution environment before passing results to the model
- **Reducing token usage**: Intermediate results stay in the execution environment, saving context window space
- **Better control flow**: Use familiar programming constructs like loops, conditionals, and error handling

For example, instead of making sequential tool calls that pass large datasets through the model's context window, an agent can write:

```typescript
const sheet = await gdrive.getSheet({ sheetId: 'abc123' });
const pendingOrders = sheet.filter(row => row.status === 'pending');
console.log(`Found ${pendingOrders.length} pending orders`);
```

This approach dramatically reduces token consumption and improves agent performance, especially when working with multiple MCP servers or large datasets.

## Features

- **Multi-server aggregation**: Connect to multiple MCP servers through a single gateway
- **Code mode interface**: Tools exposed as TypeScript functions for efficient agent interaction
- **OAuth 2.1 support**: Full compliance with MCP authorization spec including PKCE and automatic token refresh
- **Multiple auth methods**: Environment variables, system keychain, external commands
- **Secure credential handling**: Credentials never exposed to AI models

## Quick Start

```bash
# Initialize configuration
pctx init

# Add an MCP server with OAuth 2.1 authentication
pctx mcp add my-server https://mcp.example.com
pctx mcp auth my-server

# Start the gateway
pctx start --port 8080
```

## Installation

```bash
cargo install pctx
```

Or build from source:

```bash
git clone https://github.com/yourusername/pctx.git
cd pctx
cargo build --release
```

## Usage

### Initialize pctx

Create the configuration directory and files:

```bash
pctx init
```

### Managing MCP Servers

Add a new MCP server:

```bash
# Without authentication
pctx mcp add local http://localhost:3000/mcp

# With OAuth 2.1 (configure later)
pctx mcp add prod https://mcp.example.com
```

Configure authentication:

```bash
pctx mcp auth my-server
```

List all configured servers:

```bash
pctx mcp list
```

Get server details:

```bash
pctx mcp get my-server
```

Test server connection:

```bash
pctx mcp test my-server
```

Remove a server:

```bash
pctx mcp remove my-server
```

### Starting the Gateway

Start the pctx gateway server:

```bash
# Default (localhost:8080)
pctx start

# Custom port
pctx start --port 3000

# Bind to all interfaces
pctx start --host 0.0.0.0
```

The gateway exposes a single MCP endpoint at `/mcp` that provides access to tools from all configured servers as TypeScript functions.

## Authentication Methods

pctx supports multiple authentication methods:

### OAuth 2.1 (Recommended)
- Automatic discovery of authorization endpoints
- PKCE-protected authorization flow
- Automatic token refresh
- Full MCP authorization spec compliance

### Environment Variable
Reference environment variables with `${VAR_NAME}` syntax:
```bash
pctx mcp add my-server https://api.example.com --auth env --auth-token MY_TOKEN_VAR
```

### System Keychain
Secure storage in OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service):
```bash
pctx mcp add my-server https://api.example.com --auth keychain --auth-account my-account
```

### External Command
Run any command that outputs a token:
```bash
pctx mcp add my-server https://api.example.com --auth command --auth-command "op read op://vault/server/token"
```

## Learn More

- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)
- [Code Mode explanation by Cloudflare](https://blog.cloudflare.com/code-mode-mcp)
- [Code execution with MCP by Anthropic](https://www.anthropic.com/research/code-execution-mcp)

## License

MIT
