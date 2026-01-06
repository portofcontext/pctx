# pctx Benchmarks

Run [MCP-Bench](https://github.com/Accenture/mcp-bench) using pctx's unified MCP model.

## Quick Start

```bash
# Set API key
export OPENROUTER_BENCHMARK_KEY=your_key_here

# Run benchmarks (from repo root)
make bench

# Customize model and task count
make bench MODEL=anthropic/claude-3.5-sonnet TASKS=20
```

## Configuration

Edit [mcp_servers.json](mcp_servers.json) to configure MCP servers:

```json
{
  "servers": [
    {
      "name": "Wikipedia",
      "command": "npx",
      "args": ["-y", "@shelm/wikipedia-mcp-server"],
      "env": {}
    }
  ]
}
```

## Results

Results saved to:
- `data/benchmark_results.json` - Aggregated metrics
- `data/runs/<timestamp>/` - Per-task debug artifacts
