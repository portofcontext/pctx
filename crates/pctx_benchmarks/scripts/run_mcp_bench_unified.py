#!/usr/bin/env python3
"""
Run MCP-Bench using pctx's unified MCP model with Langchain agents.

This uses pctx as an MCP server exposing list_functions, get_function_details, and execute.
The LLM agent uses these tools following the intended workflow.
"""

import argparse
import asyncio
import json
import os
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Any

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import HumanMessage, SystemMessage, ToolMessage
    from langchain_core.tools import StructuredTool
except ImportError as e:
    print(f"Error: {e}")
    print("Install: pip install langchain langchain-openai langchain-core")
    sys.exit(1)

try:
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client
except ImportError as e:
    print(f"Error: {e}")
    print("Install: pip install mcp")
    sys.exit(1)


def load_mcp_server_configs() -> dict:
    """Load MCP server configs from mcp_servers.json."""
    config_file = Path(__file__).parent.parent / "mcp_servers.json"
    if not config_file.exists():
        # Fallback to defaults if file doesn't exist
        return {
            "Wikipedia": {
                "name": "Wikipedia",
                "command": "npx",
                "args": ["-y", "@shelm/wikipedia-mcp-server"],
                "env": {},
            }
        }

    with open(config_file) as f:
        data = json.load(f)
        return data.get("servers", {})


def create_pctx_config(servers: list[dict], config_path: Path):
    """Create pctx.json config with environment variable substitution."""
    # Substitute environment variables in server configs
    servers_with_env = []
    for server in servers:
        server_copy = server.copy()
        if "env" in server_copy and server_copy["env"]:
            env_substituted = {}
            for key, value in server_copy["env"].items():
                # Substitute ${VAR_NAME} with actual environment variable
                if isinstance(value, str) and value.startswith("${") and value.endswith("}"):
                    env_var_name = value[2:-1]
                    env_substituted[key] = os.environ.get(env_var_name, "")
                else:
                    env_substituted[key] = value
            server_copy["env"] = env_substituted
        servers_with_env.append(server_copy)

    config = {
        "name": "pctx-benchmark",
        "version": "0.1.0",
        "servers": servers_with_env,
    }
    with open(config_path, "w") as f:
        json.dump(config, f, indent=2)


async def run_task(
    task: dict[str, Any],
    model_name: str,
    openrouter_key: str,
    pctx_config_path: Path,
    verbose: bool = False,
) -> dict[str, Any]:
    """Run a benchmark task using pctx MCP server and Langchain agent."""

    task_id = task["task_id"]
    task_description = task["task_description"]

    if verbose:
        print(f"\n{'=' * 60}")
        print(f"Task: {task_id}")
        print(f"Description: {task_description[:150]}...")
        print(f"{'=' * 60}\n")

    start_time = time.time()

    # Track detailed execution info
    execution_log = {
        "tool_calls": [],
        "generated_code": [],
        "timing": {},
    }

    # Start pctx MCP server
    server_params = StdioServerParameters(
        command="pctx",
        args=["mcp", "start", "--stdio", "--config", str(pctx_config_path)],
        env=None,
    )

    async with stdio_client(server_params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()

            # Create tool wrappers with tracking
            async def list_functions_tool() -> str:
                """List all available MCP functions."""
                tool_start = time.time()
                result = await session.call_tool("list_functions", {})
                tool_time = int((time.time() - tool_start) * 1000)

                execution_log["tool_calls"].append(
                    {
                        "tool": "list_functions",
                        "args": {},
                        "execution_time_ms": tool_time,
                    }
                )
                return result.content[0].text if result.content else ""

            async def get_function_details_tool(functions: str) -> str:
                """Get detailed type information for specific functions.

                Args:
                    functions: Comma-separated function names like 'Wikipedia.search,Wikipedia.getPage'
                """
                tool_start = time.time()
                func_list = [f.strip() for f in functions.split(",")]
                result = await session.call_tool(
                    "get_function_details", {"functions": func_list}
                )
                tool_time = int((time.time() - tool_start) * 1000)

                execution_log["tool_calls"].append(
                    {
                        "tool": "get_function_details",
                        "args": {"functions": functions},
                        "execution_time_ms": tool_time,
                    }
                )
                return result.content[0].text if result.content else ""

            async def execute_tool(code: str) -> str:
                """Execute TypeScript code that calls MCP functions.

                Args:
                    code: TypeScript code with async function run() that returns result
                """
                tool_start = time.time()
                result = await session.call_tool("execute", {"code": code})
                tool_time = int((time.time() - tool_start) * 1000)

                execution_log["tool_calls"].append(
                    {
                        "tool": "execute",
                        "args": {"code": code},
                        "result": result.content[0].text if result.content else "",
                        "execution_time_ms": tool_time,
                    }
                )
                execution_log["generated_code"].append(code)
                return result.content[0].text if result.content else ""

            # Create Langchain tools
            tools = [
                StructuredTool.from_function(
                    coroutine=list_functions_tool,
                    name="list_functions",
                    description="ALWAYS USE THIS FIRST. Lists all available MCP functions with signatures. Returns TypeScript namespace declarations.",
                ),
                StructuredTool.from_function(
                    coroutine=get_function_details_tool,
                    name="get_function_details",
                    description="Get detailed parameter types for specific functions. Pass comma-separated function names like 'Wikipedia.search,Wikipedia.getPage'. Returns full TypeScript type definitions.",
                ),
                StructuredTool.from_function(
                    coroutine=execute_tool,
                    name="execute",
                    description="Execute TypeScript code. Code must define: async function run() { /* your code */ return result; }. Functions are called as Namespace.functionName({param: value}). Returns the result from run().",
                ),
            ]

            # Create LLM with tool binding
            llm = ChatOpenAI(
                model=model_name,
                openai_api_key=openrouter_key,
                openai_api_base="https://openrouter.ai/api/v1",
                temperature=0,
            ).bind_tools(tools)

            # Run agentic loop
            try:
                messages = [
                    SystemMessage(
                        content="""You are a TypeScript coding assistant that completes tasks using MCP tools.

WORKFLOW:
1. Call list_functions() to see available functions
2. Call get_function_details() for functions you need (comma-separated)
3. Write TypeScript code and call execute() to run it

TYPESCRIPT CODE RULES:
- Must define: async function run() { return result; }
- Call functions: await Namespace.functionName({param: value})
- Return values are JavaScript objects (not strings)
- Keep return values small and focused

Complete the task efficiently."""
                    ),
                    HumanMessage(content=f"Complete this task:\n\n{task_description}"),
                ]

                max_iterations = 15
                final_output = ""
                total_input_tokens = 0
                total_output_tokens = 0
                iteration_count = 0

                for iteration in range(max_iterations):
                    iteration_count = iteration + 1

                    # Get LLM response
                    iteration_start = time.time()
                    response = await llm.ainvoke(messages)
                    iteration_time = int((time.time() - iteration_start) * 1000)

                    # Track token usage from response metadata
                    if hasattr(response, "response_metadata"):
                        metadata = response.response_metadata
                        if "token_usage" in metadata:
                            token_usage = metadata["token_usage"]
                            total_input_tokens += token_usage.get("prompt_tokens", 0)
                            total_output_tokens += token_usage.get(
                                "completion_tokens", 0
                            )

                    messages.append(response)

                    # Check if done (no tool calls)
                    if not response.tool_calls:
                        final_output = response.content
                        execution_log["timing"][f"iteration_{iteration}"] = (
                            iteration_time
                        )
                        break

                    # Execute tool calls
                    for tool_call in response.tool_calls:
                        tool_name = tool_call["name"]
                        tool_args = tool_call["args"]

                        # Find and execute the tool
                        tool_result = None
                        for tool in tools:
                            if tool.name == tool_name:
                                # Use ainvoke which handles async tools properly
                                tool_result = await tool.ainvoke(tool_args)
                                break

                        # Add tool result to messages
                        messages.append(
                            ToolMessage(
                                content=str(tool_result),
                                tool_call_id=tool_call["id"],
                            )
                        )

                    execution_log["timing"][f"iteration_{iteration}"] = iteration_time

                execution_time_ms = int((time.time() - start_time) * 1000)

                return {
                    "task_id": task_id,
                    "success": True,
                    "execution_time_ms": execution_time_ms,
                    "output": final_output,
                    "error": "",
                    "iterations": iteration_count,
                    "token_usage": {
                        "input_tokens": total_input_tokens,
                        "output_tokens": total_output_tokens,
                        "total_tokens": total_input_tokens + total_output_tokens,
                    },
                    "execution_log": execution_log,
                }

            except Exception as e:
                execution_time_ms = int((time.time() - start_time) * 1000)
                return {
                    "task_id": task_id,
                    "success": False,
                    "execution_time_ms": execution_time_ms,
                    "output": "",
                    "error": str(e),
                    "iterations": 0,
                    "token_usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "total_tokens": 0,
                    },
                    "execution_log": execution_log,
                }


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--openrouter-key", default=os.environ.get("OPENROUTER_BENCHMARK_KEY")
    )
    parser.add_argument("--model", default="deepseek/deepseek-chat")
    parser.add_argument("--max-tasks", type=int, help="Maximum number of tasks to run")
    parser.add_argument("--task-id", help="Specific task ID to run")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    if not args.openrouter_key:
        print("Error: OpenRouter API key required")
        sys.exit(1)

    # Load MCP server configs first to know what's available
    mcp_server_configs = load_mcp_server_configs()
    available_servers = set(mcp_server_configs.keys())

    # Load tasks
    dataset_path = (
        Path(__file__).parent.parent / "data/mcpbench_tasks_single_runner_format.json"
    )
    with open(dataset_path) as f:
        data = json.load(f)

    all_tasks = []
    skipped_count = 0
    for server_entry in data["server_tasks"]:
        for task in server_entry["tasks"]:
            task["servers"] = server_entry["servers"]
            # Skip tasks that require servers we don't have configured
            required_servers = set(server_entry["servers"])
            if required_servers.issubset(available_servers):
                all_tasks.append(task)
            else:
                skipped_count += 1
                missing_servers = required_servers - available_servers
                if args.verbose:
                    print(f"Skipping {task['task_id']} - missing servers: {', '.join(missing_servers)}")

    if skipped_count > 0:
        print(f"Skipped {skipped_count} tasks due to missing server configurations")

    # Filter
    if args.task_id:
        all_tasks = [t for t in all_tasks if t["task_id"] == args.task_id]
    elif args.max_tasks:
        all_tasks = all_tasks[: args.max_tasks]

    if not all_tasks:
        print("No tasks found")
        sys.exit(1)

    task = all_tasks[0]

    # Create pctx config
    config_path = Path("/tmp/pctx_benchmark.json")
    servers_config = []
    for server_name in task["servers"]:
        if server_name in mcp_server_configs:
            servers_config.append(mcp_server_configs[server_name])

    create_pctx_config(servers_config, config_path)

    print(f"Running task: {task['task_id']}")
    print(f"Servers: {', '.join(task['servers'])}\n")

    # Run
    result = await run_task(
        task=task,
        model_name=args.model,
        openrouter_key=args.openrouter_key,
        pctx_config_path=config_path,
        verbose=args.verbose,
    )

    # Save detailed results
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    results_dir = Path(__file__).parent.parent / "data" / "runs" / timestamp
    results_dir.mkdir(parents=True, exist_ok=True)

    # Save full result with all details
    result_file = results_dir / f"{task['task_id']}.json"

    # Create formatted output with code blocks saved to separate files
    execution_log = result.get("execution_log", {})
    generated_code = execution_log.get("generated_code", [])

    # Save generated code blocks to separate files for readability
    code_files = []
    for i, code_block in enumerate(generated_code):
        code_file = results_dir / f"{task['task_id']}_code_{i}.ts"
        with open(code_file, "w") as cf:
            cf.write(code_block)
        code_files.append(str(code_file.relative_to(results_dir)))

    # Update execution log to reference code files instead of inline code
    execution_log_for_json = execution_log.copy()
    if code_files:
        execution_log_for_json["generated_code_files"] = code_files
        # Keep first 200 chars of each code block for preview
        execution_log_for_json["generated_code_preview"] = [
            code[:200] + ("..." if len(code) > 200 else "") for code in generated_code
        ]

    # Prepare result with formatted execution log
    result_for_json = result.copy()
    result_for_json["execution_log"] = execution_log_for_json

    with open(result_file, "w") as f:
        json.dump(
            {
                "task_id": task["task_id"],
                "task_description": task["task_description"],
                "servers": task["servers"],
                "model": args.model,
                "timestamp": timestamp,
                "result": result_for_json,
            },
            f,
            indent=2,
        )

    # Print summary
    print(f"\n{'=' * 60}")
    print(f"Result: {'✓ SUCCESS' if result['success'] else '✗ FAILED'}")
    print(f"Time: {result['execution_time_ms']}ms")
    print(f"Iterations: {result.get('iterations', 0)}")

    # Print token usage
    token_usage = result.get("token_usage", {})
    if token_usage.get("total_tokens", 0) > 0:
        print(
            f"Tokens: {token_usage['input_tokens']} input + {token_usage['output_tokens']} output = {token_usage['total_tokens']} total"
        )

    # Print tool usage summary
    execution_log = result.get("execution_log", {})
    tool_calls = execution_log.get("tool_calls", [])
    if tool_calls:
        print(f"Tool calls: {len(tool_calls)}")
        list_fn_count = sum(1 for t in tool_calls if t["tool"] == "list_functions")
        get_details_count = sum(
            1 for t in tool_calls if t["tool"] == "get_function_details"
        )
        execute_count = sum(1 for t in tool_calls if t["tool"] == "execute")
        print(f"  - list_functions: {list_fn_count}")
        print(f"  - get_function_details: {get_details_count}")
        print(f"  - execute: {execute_count}")

    # Print generated code count
    if code_files:
        print(f"Generated code blocks: {len(code_files)}")
        for code_file in code_files:
            print(f"  - {results_dir / code_file}")

    if result["output"]:
        print(f"\nOutput: {result['output'][:300]}")
    if result["error"]:
        print(f"\nError: {result['error']}")

    print(f"\n{'=' * 60}")
    print(f"Results saved to: {results_dir}")
    print(f"  - Task result: {result_file.name}")
    if code_files:
        for cf in code_files:
            print(f"  - Generated code: {cf}")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    asyncio.run(main())
