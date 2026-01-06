#!/usr/bin/env -S uv run --script
# /// script
# dependencies = [
#   "langchain>=0.1.0",
#   "langchain-openai>=0.0.5",
#   "langchain-core>=0.1.0",
#   "mcp>=0.1.0",
#   "python-dotenv>=1.0.0",
# ]
# ///
"""
Run MCP-Bench using pctx's unified MCP model.

This adapts mcp-bench to use `pctx mcp start --stdio` instead of connecting
to individual MCP servers directly. All MCP servers are accessed through pctx's
unified interface (list_functions, get_function_details, execute).
"""

import argparse
import asyncio
import json
import os
import shlex
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
    print("Install: uv pip install langchain langchain-openai langchain-core")
    sys.exit(1)

try:
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client
except ImportError as e:
    print(f"Error: {e}")
    print("Install: uv pip install mcp")
    sys.exit(1)

try:
    from dotenv import load_dotenv
except ImportError as e:
    print(f"Error: {e}")
    print("Install: uv pip install python-dotenv")
    sys.exit(1)


def load_env_file():
    """Load environment variables from .env file in benchmarks directory."""
    env_file = Path(__file__).parent / ".env"
    if env_file.exists():
        load_dotenv(env_file)
        print(f"Loaded environment from {env_file}")
    else:
        print(f"Warning: No .env file found at {env_file}")


def convert_commands_to_pctx_config(commands_json_path: Path) -> dict:
    """
    Convert mcp-bench's commands.json format to pctx server configuration.

    mcp-bench format:
    {
      "Wikipedia": {
        "cmd": "uv run python -m wikipedia_mcp",
        "env": [],
        "cwd": "../wikipedia-mcp"
      }
    }

    pctx format:
    {
      "servers": [
        {
          "name": "Wikipedia",
          "command": "uv",
          "args": ["run", "python", "-m", "wikipedia_mcp"],
          "env": {}
        }
      ]
    }
    """
    with open(commands_json_path) as f:
        commands = json.load(f)

    servers = []
    for server_name, config in commands.items():
        # Parse command string into command + args
        cmd_parts = config["cmd"].split()
        command = cmd_parts[0]
        args = cmd_parts[1:] if len(cmd_parts) > 1 else []

        # Convert env list to dict with values from environment
        env_dict = {}
        for env_var in config.get("env", []):
            env_dict[env_var] = os.environ.get(env_var, "")

        # Handle cwd - pctx doesn't support cwd field for stdio servers
        # So we need to wrap the command with a cd command
        if "cwd" in config:
            # The cwd in commands.json uses "../server-name" format
            # but the servers are actually in mcp_servers/ directory
            # So we strip the "../" prefix
            cwd_rel = config["cwd"]
            if cwd_rel.startswith("../"):
                cwd_rel = cwd_rel[3:]  # Remove "../"

            cwd_path = Path(__file__).parent / "mcp-bench" / "mcp_servers" / cwd_rel
            if not cwd_path.exists():
                # Try without modification
                cwd_path = Path(__file__).parent / "mcp-bench" / "mcp_servers" / config["cwd"]

            # Wrap command with shell that cd's first
            # Create a shell command that changes directory then executes the original command
            cwd_str = str(cwd_path.resolve())
            if command == "node" or command == "python" or command == "uv":
                # For interpreted languages, prepend cd to args via shell
                shell_command = f"cd {shlex.quote(cwd_str)} && {command} {' '.join(shlex.quote(arg) for arg in args)}"
                command = "sh"
                args = ["-c", shell_command]
            else:
                # For other commands, do the same
                shell_command = f"cd {shlex.quote(cwd_str)} && {command} {' '.join(shlex.quote(arg) for arg in args)}"
                command = "sh"
                args = ["-c", shell_command]

        server_config = {
            "name": server_name,
            "command": command,
            "args": args,
            "env": env_dict
        }

        servers.append(server_config)

    return {"servers": servers}


def create_pctx_config_for_servers(server_names: list[str], output_path: Path):
    """Create a pctx config with only the specified servers."""
    commands_json = Path(__file__).parent / "mcp-bench" / "mcp_servers" / "commands.json"

    if not commands_json.exists():
        raise FileNotFoundError(f"commands.json not found at {commands_json}")

    # Load and convert all servers
    all_servers_config = convert_commands_to_pctx_config(commands_json)

    # Filter to only requested servers
    filtered_servers = [
        s for s in all_servers_config["servers"]
        if s["name"] in server_names
    ]

    if len(filtered_servers) != len(server_names):
        found_names = {s["name"] for s in filtered_servers}
        missing = set(server_names) - found_names
        raise ValueError(f"Servers not found in commands.json: {missing}")

    config = {
        "name": "pctx-benchmark",
        "version": "0.1.0",
        "servers": filtered_servers
    }

    with open(output_path, "w") as f:
        json.dump(config, f, indent=2)

    return config


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

            # Validate that MCP servers started successfully
            try:
                initial_check = await session.call_tool("list_functions", {})
                functions_list = initial_check.content[0].text if initial_check.content else ""

                if not functions_list or functions_list.strip() == "":
                    # No functions available - servers failed to start
                    raise RuntimeError(
                        f"MCP servers failed to start. No functions available. "
                        f"Expected servers: {task.get('servers', [])}. "
                        f"This usually indicates a Python version incompatibility or missing dependencies."
                    )

                # Check that all expected servers are present
                expected_servers = task.get("servers", [])
                for server_name in expected_servers:
                    # Normalize server name by removing spaces for namespace check
                    # e.g., "Unit Converter" -> "UnitConverter"
                    namespace_name = server_name.replace(" ", "")
                    if f"namespace {namespace_name}" not in functions_list:
                        raise RuntimeError(
                            f"Expected MCP server '{server_name}' not found in list_functions output. "
                            f"Server may have failed to start. Check Python version and dependencies."
                        )

                if verbose:
                    print(f"✓ All {len(expected_servers)} MCP server(s) started successfully")

            except Exception as e:
                raise RuntimeError(f"MCP server startup validation failed: {e}")

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
                        content="""You are an autonomous TypeScript coding assistant that completes benchmark tasks using MCP tools.

CRITICAL INSTRUCTIONS:
- Work AUTONOMOUSLY - do NOT ask for permission or confirmation
- Execute all steps immediately without waiting for user approval
- Follow the task description exactly as specified
- Complete the entire task in one session
- You MUST use the execute() tool to actually run code - calling list_functions() alone does NOTHING

WORKFLOW (MANDATORY):
1. Call list_functions() to see available functions
2. IMMEDIATELY write TypeScript code and call execute() to run the first step
3. Continue calling execute() for each subsequent step until task is complete
4. You can optionally call get_function_details() if you need type information

HOW TO USE execute():
The execute() tool is the ONLY way to actually call MCP functions. Here's the exact pattern:

Step 1 - After calling list_functions(), you see: "namespace Wikipedia { export async function searchWikipedia(...) }"
Step 2 - To actually search, you MUST call the execute() tool like this:
  Tool: execute
  Args: {
    "code": "async function run() {\n  const results = await Wikipedia.searchWikipedia({query: 'climate change', limit: 5});\n  return results;\n}"
  }

Step 3 - The execute() tool will run your TypeScript code and return the results

EXAMPLE COMPLETE FLOW:
1. list_functions() returns: "namespace Wikipedia { export async function searchWikipedia(...) }"
2. execute(code="async function run() { return await Wikipedia.searchWikipedia({query: 'test', limit: 5}); }")
3. Read the results from execute() response
4. execute(code="async function run() { return await Wikipedia.getArticle({title: 'Some Title'}); }")
5. Continue until task complete

TYPESCRIPT CODE RULES:
- Must define: async function run() { return result; }
- Call functions: await Namespace.functionName({param: value})
- You can make multiple function calls in a single execute() call
- Store intermediate results in variables: const data = await Wikipedia.searchWikipedia(...)

IMPORTANT:
- Never ask "Would you like me to proceed?" - just proceed
- Never ask "Should I continue?" - just continue
- Never say queries "aren't returning results" without actually calling execute() first
- If you describe what you're going to do, you MUST then call execute() to actually do it
- Execute all steps in the task description completely
- Only stop when the task is fully completed"""
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
                        if verbose:
                            print(f"\nIteration {iteration}: Model stopped without tool calls")
                            print(f"Response: {response.content[:200]}...")
                        break

                    # Execute tool calls
                    for tool_call in response.tool_calls:
                        tool_name = tool_call["name"]
                        tool_args = tool_call["args"]

                        if verbose:
                            print(f"\nIteration {iteration}: Calling tool '{tool_name}'")
                            if tool_name == "execute" and "code" in tool_args:
                                print(f"Code preview: {tool_args['code'][:100]}...")

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
    parser = argparse.ArgumentParser(
        description="Run MCP-Bench using pctx unified MCP interface"
    )
    parser.add_argument(
        "--openrouter-key",
        default=os.environ.get("OPENROUTER_API_KEY"),
        help="OpenRouter API key (or set OPENROUTER_API_KEY env var)",
    )
    parser.add_argument(
        "--model",
        default="deepseek/deepseek-chat",
        help="Model to use (default: deepseek/deepseek-chat)",
    )
    parser.add_argument(
        "--max-tasks", type=int, help="Maximum number of tasks to run"
    )
    parser.add_argument("--task-id", help="Specific task ID to run")
    parser.add_argument(
        "--tasks-file",
        default="mcp-bench/tasks/mcpbench_tasks_single_runner_format.json",
        help="Path to tasks file relative to benchmarks directory",
    )
    parser.add_argument("--verbose", action="store_true", help="Verbose output")
    args = parser.parse_args()

    # Load environment variables
    load_env_file()

    # Re-check for API key after loading .env
    if not args.openrouter_key:
        args.openrouter_key = os.environ.get("OPENROUTER_API_KEY")

    if not args.openrouter_key:
        print("Error: OpenRouter API key required")
        print("Set OPENROUTER_API_KEY in .env file or pass --openrouter-key")
        sys.exit(1)

    # Load tasks
    tasks_path = Path(__file__).parent / args.tasks_file
    if not tasks_path.exists():
        print(f"Error: Tasks file not found at {tasks_path}")
        sys.exit(1)

    with open(tasks_path) as f:
        data = json.load(f)

    all_tasks = []
    for server_entry in data["server_tasks"]:
        for task in server_entry["tasks"]:
            task["servers"] = server_entry["servers"]
            all_tasks.append(task)

    # Filter tasks
    if args.task_id:
        all_tasks = [t for t in all_tasks if t["task_id"] == args.task_id]
    elif args.max_tasks:
        all_tasks = all_tasks[: args.max_tasks]

    if not all_tasks:
        print("No tasks found")
        sys.exit(1)

    # For now, run first task
    task = all_tasks[0]

    # Create pctx config for this task's servers
    config_path = Path("/tmp/pctx_benchmark.json")
    try:
        create_pctx_config_for_servers(task["servers"], config_path)
    except Exception as e:
        print(f"Error creating pctx config: {e}")
        sys.exit(1)

    print(f"Running task: {task['task_id']}")
    print(f"Servers: {', '.join(task['servers'])}")
    print(f"Model: {args.model}\n")

    # Run task
    result = await run_task(
        task=task,
        model_name=args.model,
        openrouter_key=args.openrouter_key,
        pctx_config_path=config_path,
        verbose=args.verbose,
    )

    # Save detailed results
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    results_dir = Path(__file__).parent / "results" / timestamp
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
