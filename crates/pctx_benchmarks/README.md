# pctx_benchmarks

Run MCP-Bench benchmarks using pctx with OpenRouter.

## Status

⚠️ **Blocked**: pctx currently only supports HTTP-based MCP servers, but most MCP servers (including those required for MCP-Bench) use stdio transport.

**Next steps**:
1. Add stdio MCP server support to pctx, OR
2. Use an HTTP proxy/adapter for stdio MCP servers, OR
3. Find/create HTTP-based versions of required MCP servers

The benchmark framework is complete and ready to use once pctx supports stdio MCP servers.

## Quick Start

```bash
# 1. Download dataset
cargo run --bin benchmark -p pctx_benchmarks download

# 2. Start pctx server (in separate terminal)
# TODO: Configure MCP servers in server config first
pctx server start

# 3. Run benchmark
cargo run --bin benchmark -p pctx_benchmarks mcp --openrouter-key YOUR_KEY
```

## How It Works

The benchmark:
1. Downloads MCP-Bench tasks from the official repository
2. Connects to a running pctx server (which must have MCP servers configured)
3. Uses an LLM (via OpenRouter) to generate TypeScript code
4. Executes the code via pctx to complete the task
5. Reports success/failure rates

## Requirements

- **pctx server**: Must be running with MCP servers configured
- **Python 3.8+**: Python packages auto-installed via pip
- **OpenRouter API key**: Get one at https://openrouter.ai/settings/keys

## MCP Server Configuration

MCP servers must be configured server-side in the pctx server configuration. The Python client cannot dynamically spawn MCP servers - they must be pre-configured when starting the server.

### Required MCP Servers for MCP-Bench

Tasks require various MCP servers:
- **OpenAPI Explorer**: For API exploration tasks
- **Unit Converter**: For unit conversion tasks
- **Wikipedia**: For knowledge retrieval tasks
- **Google Maps**: For location-based tasks (requires `GOOGLE_MAPS_API_KEY`)
- And many more...

See [MCP-Bench repository](https://github.com/Accenture/mcp-bench) for the complete list of 28 MCP servers.

## Current Limitations

1. **Manual server configuration**: MCP servers must be manually configured on the pctx server
2. **No dynamic spawning**: The Python client cannot spawn MCP servers on demand
3. **Server-side dependency**: Requires a running pctx server with pre-configured MCP servers

## Results

View results in `data/benchmark_results.json`

## Implementation Notes

### Fixes Applied

- **Code fence stripping**: Handles LLM output wrapped in markdown code fences ([run_mcp_bench.py:98-109](scripts/run_mcp_bench.py#L98-L109))
- **Biome formatter bug**: Template literals may appear mangled in debug logs, but execution uses original code ([code_mode.rs:116-117](../../pctx_code_mode/src/code_mode.rs#L116-L117))

### Architecture

The pctx client-server architecture requires:
- **Server side**: Spawns and manages MCP server processes
- **Client side**: Connects to running pctx server and executes code

This means MCP servers cannot be dynamically configured per-task from the Python client.
