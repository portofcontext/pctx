<div align="center">
  <img src=".github/assets/logo.png" alt="PCTX Logo" style="height: 128px">
</div>

# PCTX

<div align="center">

[![Crates.io](https://img.shields.io/crates/v/pctx.svg)](https://crates.io/crates/pctx)
[![Documentation](https://docs.rs/pctx/badge.svg)](https://docs.rs/pctx)
[![License](https://img.shields.io/crates/l/pctx.svg)](https://github.com/portofcontext/pctx/blob/main/LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.89%2B-blue.svg)](https://www.rust-lang.org)
[![CI](https://github.com/portofcontext/pctx/workflows/CI/badge.svg)](https://github.com/portofcontext/pctx/actions)
[![Downloads](https://img.shields.io/crates/d/pctx.svg)](https://crates.io/crates/pctx)
[![dependency status](https://deps.rs/repo/github/portofcontext/pctx/status.svg)](https://deps.rs/repo/github/portofcontext/pctx)

The open source framework to connect AI agents to tools and services with [code mode](#what-is-pctx-and-what-is-code-mode)


</div>


# Getting Started

- Visit our [learn pctx](todo) course to get started.
- Visit the [pctx showcase](todo) to see live mcp servers built with `pctx`.

# Documentation

Visit https://portofcontext.com/docs to view the full documentation.

## Installation

```bash
brew install pctx
npm
choco
```

## Quick Start

```bash
# Initialize configuration for auth and mcp host management
pctx init
# Add an MCP server
pctx mcp add my-server https://mcp.example.com
# Start the gateway
pctx start
```

## What is pctx and what is Code Mode?

Unlike traditional MCP implementations where agents directly call tools, `pctx` generates code and uses code mode to expose MCP tools as TypeScript functions. This allows AI agents to write code that calls MCP servers more efficiently by:

- **Loading tools on-demand**: Only load the interfaces needed for the current task, rather than all tools upfront like in traditional tool calling.
- **Reducing token usage**: Intermediate results stay in the execution environment, saving context window space.
- **Better control flow**: Use programming constructs like loops, conditionals, and error handling

#### Quick Example
Instead of making sequential tool calls that pass data through the context window, an agent can write:

```typescript
const sheet = await gdrive.getSheet({ sheetId: 'abc' });
const orders = sheet.filter(row => row.status === 'pending');
console.log(`Found ${order.length} orders`);
```

This example reduces the token usage from 150,000 tokens to 2,000 tokens leading to a **time and cost saving of 98.7%**.

## Features

- **Code mode interface**: Tools exposed as TypeScript functions for efficient agent interaction. See [Code Mode Guide](docs/code-mode.md).
- **Upstream MCP server aggregation**: Connect to multiple MCP servers through a single gateway. See [Upstream MCP Servers Guide](docs/upstream-mcp-servers.md).
- **Secure authentication**: OAuth 2.1, environment variables, system keychain, and external commands. See [Authentication Guide](docs/mcp-auth.md).


### Architecture

```
       ┌─────────────────────────────────┐
       │      AI Agents (Any LLM)        │
       └────────────┬────────────────────┘
                    │ MCP Protocol
       ┌────────────▼────────────────────┐
       │            pctx                 │
       │                                 │
       │  • MCP Server to Agents         │
       │  • Auth & Route Management      │
       │  • "Code Mode" Sandbox Env      │
       │  • Client to MCP Servers        │
       └──┬──────┬──────┬──────┬─────────┘
          │      │      │      │
          ↓      ↓      ↓      ↓
       ┌──────┬──────┬──────┬──────┐
       │GDrive│Slack │GitHub│Custom│
       └──────┴──────┴──────┴──────┘

       Runs locally • in docker • any cloud
```


### Security

- Code runs in an isolated [Deno](https://deno.com) sandbox that can only access the network hosts specified in the configuration file.
- No filesystem, environment, network (beyond allowed hosts), or system access.
- MCP clients are authenticated. LLMs cannot view the auth.


## Learn More

- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)
- [Code Mode explanation by Cloudflare](https://blog.cloudflare.com/code-mode-mcp)
- [Code execution with MCP by Anthropic](https://www.anthropic.com/research/code-execution-mcp)

## License

MIT
