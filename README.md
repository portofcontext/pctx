<div align="center">
  <img src=".github/assets/logo.png" alt="PCTX Logo" style="height: 128px">
  <h1>pctx</h1>

[![Made by](https://img.shields.io/badge/MADE%20BY-Port%20of%20Context-1e40af.svg?style=for-the-badge&labelColor=0c4a6e)](https://portofcontext.com)

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/portofcontext/pctx/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-blue.svg)](https://www.rust-lang.org)
[![CI](https://github.com/portofcontext/pctx/workflows/CI/badge.svg)](https://github.com/portofcontext/pctx/actions)

</div>

<div align="center">

The open source framework to connect AI agents to tools and services with [code mode](#what-is-code-mode)


</div>

## Pick installation method

```bash
# Homebrew
brew install portofcontext/tap/pctx

# Curl
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/portofcontext/pctx/main/install.sh | sh

# npm and crates.io coming soon
```

## Quick Start

```bash
# Initialize config for upstream mcp connections
pctx init

# Connect to any MCP server
pctx add my-local-server http://localhost:3000/mcp
pctx add stripe https://mcp.stripe.com

# Start the unified MCP server
pctx start
```

For complete CLI documentation, see [CLI.md](docs/CLI.md).

## What is pctx?

`pctx` sits between AI agents and MCP servers. It aggregates multiple upstream MCP servers, handles authentication, and exposes tools through a unified Code Mode interface. Instead of agents managing connections to individual MCP servers, they connect once to pctx.

## What is Code Mode?

Code mode replaces sequential tool calling with code execution. Rather than an agent calling tools one at a time and passing results through its context window, it writes TypeScript code that executes in a sandbox. Read Anthropic's overview [here](https://www.anthropic.com/engineering/code-execution-with-mcp).

**Traditional MCP flow**:
1. Agent calls `getSheet(id)`
2. Server returns 1000 rows → agent's context
3. Agent calls `filterRows(criteria)`
4. Server returns 50 rows → agent's context

**With Code Mode**:
```typescript
const sheet = await gdrive.getSheet({ sheetId: 'abc' });
const orders = sheet.filter(row => row.status === 'pending');
console.log(`Found ${orders.length} orders`);
```

**Result:** 98.7% reduction in tokens (150k → 2k) for this multi-step operation.

## Features

- **Code Mode interface**: Tools exposed as TypeScript functions for efficient agent interaction. See [Code Mode Guide](docs/code-mode.md).
- **Upstream MCP server aggregation**: Connect to multiple MCP servers through a single gateway. See [Upstream MCP Servers Guide](docs/upstream-mcp-servers.md).
- **Secure authentication**: OAuth, environment variables, system keychain, and external commands. See [Authentication Guide](docs/mcp-auth.md).

## Architecture

```
    Runs locally • in docker • any cloud

  ┌─────────────────────────────────┐
  │      AI Agents (Bring any LLM)  │
  └──────────────-──────────────────┘
                │ MCP
                │ • list_functions
                │ • get_function_details
                │ • execute
  ┌─────────────▼───────────────────┐
  │            pctx                 │
  │                                 │
  │  ┌─────────────────────────┐    │
  │  │  TypeScript Compiler    │    │
  │  │  Sandbox (Deno)         │    │
  │  │                         │    │
  │  │  • Type checking        │    │
  │  │  • Rich error feedback  │    │
  │  │  • No network access    │    │
  │  └──────────┬──────────────┘    │
  │             │ Compiled JS       │
  │  ┌──────────▼──────────────┐    │
  │  │  Execution Sandbox      │    │
  │  │  (Deno Runtime)         │    │
  │  │                         │    │
  │  │  • Authenticated MCP    │    │
  │  │    client connections   │    │
  │  │  • Restricted network   │    │
  │  │  • Tool call execution  │    │
  │  └──┬──────┬──────┬────────┘    │
  └─────┼──────┼──────┼─────────────┘
        │      │      │
        ↓      ↓      ↓
    ┌──────┬──────┬──────┬──────┐
    │Local │Slack │GitHub│Custom│
    └──────┴──────┴──────┴──────┘
```


## Security

- LLM generated code runs in an isolated [Deno](https://deno.com) sandbox that can only access the network hosts specified in the configuration file.
- No filesystem, environment, network (beyond allowed hosts), or system access.
- MCP clients are authenticated. LLMs cannot access auth.


## Learn More

- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)
- [Code execution with MCP by Anthropic](https://www.anthropic.com/engineering/code-execution-with-mcp)
- [Code Mode explanation by Cloudflare](https://blog.cloudflare.com/code-mode/)