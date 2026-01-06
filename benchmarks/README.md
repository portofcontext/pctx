# pctx MCP-Bench Integration

Run [MCP-Bench](https://github.com/Accenture/mcp-bench) using pctx's unified MCP model.

This integration allows you to benchmark LLM agents using pctx's `list_functions`, `get_function_details`, and `execute` workflow instead of connecting directly to individual MCP servers.

## Quick Start

```bash
# 1. Create .env file with API key
cd benchmarks
cp .env.example .env
# Edit .env and add your OPENROUTER_API_KEY

# 2. Install MCP servers (first time only)
cd ..
make bench-install

# 3. Run benchmarks
make bench
```

## Setup

### 1. Install Dependencies

The script uses uv's inline script dependencies, so it will automatically install what it needs when you run it.

Alternatively, you can pre-install dependencies:

```bash
# Using uv (recommended)
uv pip install -r requirements.txt

# Or using pip
pip install -r requirements.txt
```

### 2. Configure API Keys

Copy the example environment file and add your API keys:

```bash
cd benchmarks
cp .env.example .env
# Edit .env and add your keys
```

Required:
- `OPENROUTER_API_KEY` - For running the LLM agent

Optional (only needed for specific MCP servers):
- `NPS_API_KEY` - National Park Service
- `NASA_API_KEY` - NASA Open Data
- `HF_TOKEN` - Hugging Face
- `GOOGLE_MAPS_API_KEY` - Google Maps
- `NCI_API_KEY` - National Cancer Institute

### 3. Install MCP Servers

The mcp-bench repository includes installation scripts for all MCP servers:

```bash
cd mcp-bench/mcp_servers
bash ./install.sh
cd ../..
```

## Usage

### Using Make (Recommended)

The easiest way to run benchmarks is using the Makefile from the repo root:

```bash
# Install MCP servers (first time only)
make bench-install

# Run benchmarks (runs first task by default)
make bench

# Customize execution
make bench MODEL=anthropic/claude-3.5-sonnet TASKS=5
make bench TASK=openapi_explorer_000 VERBOSE=1

# Examples:
# Run 10 tasks with Claude Sonnet
make bench MODEL=anthropic/claude-sonnet-4 TASKS=10

# Run specific task with verbose output
make bench TASK=Wikipedia_1 VERBOSE=1
```

### Run Directly (Advanced)

You can also run the script directly from the benchmarks directory:

```bash
cd benchmarks

# Run first task from the benchmark
./run_with_pctx.py

# Or explicitly with uv
uv run run_with_pctx.py

# Run specific task by ID
./run_with_pctx.py --task-id "openapi_explorer_000"

# Use different model
./run_with_pctx.py --model anthropic/claude-3.5-sonnet

# Verbose output
./run_with_pctx.py --verbose

# Run first N tasks
./run_with_pctx.py --max-tasks 5

# Use different task file (multi-server tasks)
./run_with_pctx.py --tasks-file mcp-bench/tasks/mcpbench_tasks_multi_2server_runner_format.json
```

### Available Models

Any model from [OpenRouter](https://openrouter.ai/models) can be used. Examples:
- `anthropic/claude-3.5-sonnet`
- `anthropic/claude-sonnet-4`
- `openai/gpt-4o`
- `deepseek/deepseek-chat` (default - cost effective)
- `meta-llama/llama-3.3-70b-instruct`

## How It Works

1. **Configuration**: The script converts mcp-bench's `commands.json` format to pctx's server configuration format
2. **Execution**: Instead of connecting to each MCP server individually, pctx starts all servers and provides a unified interface
3. **Agent Workflow**: The LLM agent uses three tools:
   - `list_functions()` - Discover available MCP functions
   - `get_function_details()` - Get TypeScript type signatures for functions
   - `execute()` - Run TypeScript code that calls MCP functions
4. **Results**: Detailed execution logs, generated code, and timing information are saved to `results/`

## Results

Results are saved to `benchmarks/results/<timestamp>/`:
- `<task_id>.json` - Complete execution details, token usage, timing
- `<task_id>_code_0.ts` - Generated TypeScript code blocks

## Key Differences from Standard MCP-Bench

1. **Unified Server Access**: Uses `pctx mcp start --stdio` instead of individual MCP server connections
2. **Code Generation**: Agent writes TypeScript code to call MCP functions (via pctx's execute tool)
3. **Environment Variables**: Loaded from `.env` file instead of mcp-bench's `api_key` file
4. **Configuration**: Automatically converts mcp-bench's server commands to pctx format

## Example

```bash
# Set up environment (in benchmarks/.env)
# OPENROUTER_API_KEY=sk-or-v1-...

# Run a benchmark task
make bench TASK=openapi_explorer_000 VERBOSE=1

# Output:
# Loaded API keys from .env
# Running benchmarks with pctx...
# Running task: openapi_explorer_000
# Servers: OpenAPI Explorer
# Model: deepseek/deepseek-chat
#
# ============================================================
# Result: ✓ SUCCESS
# Time: 8234ms
# Iterations: 3
# Tokens: 1245 input + 356 output = 1601 total
# Tool calls: 5
#   - list_functions: 1
#   - get_function_details: 1
#   - execute: 3
# Generated code blocks: 2
#   - benchmarks/results/20260105_123456/openapi_explorer_000_code_0.ts
#   - benchmarks/results/20260105_123456/openapi_explorer_000_code_1.ts
# ============================================================
# Results saved to: benchmarks/results/20260105_123456
```

## Troubleshooting

### MCP Server Not Found
If you see errors about missing servers, make sure you've run the installation script:
```bash
cd mcp-bench/mcp_servers && bash ./install.sh
```

### API Key Errors
Check that your `.env` file has the required API keys and is in the `benchmarks/` directory.

### pctx Command Not Found
Make sure pctx is installed and in your PATH:
```bash
cargo install --path crates/pctx
```
