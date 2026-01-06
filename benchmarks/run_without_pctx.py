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
Run MCP-Bench WITHOUT pctx - directly connecting to individual MCP servers.

This allows comparison between pctx's unified interface and direct MCP server access.
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


def load_server_config(server_name: str) -> dict:
    """Load configuration for a specific MCP server from commands.json."""
    commands_json = Path(__file__).parent / "mcp-bench" / "mcp_servers" / "commands.json"

    if not commands_json.exists():
        raise FileNotFoundError(f"commands.json not found at {commands_json}")

    with open(commands_json) as f:
        commands = json.load(f)

    if server_name not in commands:
        raise ValueError(f"Server '{server_name}' not found in commands.json")

    config = commands[server_name]

    # Parse command string
    cmd_parts = config["cmd"].split()
    command = cmd_parts[0]
    args = cmd_parts[1:] if len(cmd_parts) > 1 else []

    # Get working directory
    cwd = None
    if "cwd" in config:
        cwd_rel = config["cwd"]
        if cwd_rel.startswith("../"):
            cwd_rel = cwd_rel[3:]  # Remove "../"
        cwd = Path(__file__).parent / "mcp-bench" / "mcp_servers" / cwd_rel
        if not cwd.exists():
            cwd = Path(__file__).parent / "mcp-bench" / "mcp_servers" / config["cwd"]

    # Build environment
    env_dict = dict(os.environ)  # Start with current environment
    for env_var in config.get("env", []):
        if env_var in os.environ:
            env_dict[env_var] = os.environ[env_var]

    return {
        "name": server_name,
        "command": command,
        "args": args,
        "cwd": str(cwd) if cwd else None,
        "env": env_dict
    }


async def run_task(
    task: dict[str, Any],
    model_name: str,
    openrouter_key: str,
    verbose: bool = False,
) -> dict[str, Any]:
    """Run a benchmark task by connecting directly to MCP servers."""

    task_id = task["task_id"]
    task_description = task["task_description"]
    server_names = task["servers"]

    if verbose:
        print(f"\n{'=' * 60}")
        print(f"Task: {task_id}")
        print(f"Description: {task_description[:150]}...")
        print(f"Servers: {', '.join(server_names)}")
        print(f"{'=' * 60}\n")

    start_time = time.time()

    # Track detailed execution info
    execution_log = {
        "tool_calls": [],
        "generated_code": [],
        "timing": {},
    }

    # Currently only support single server for simplicity
    if len(server_names) > 1:
        return {
            "task_id": task_id,
            "success": False,
            "execution_time_ms": int((time.time() - start_time) * 1000),
            "output": "",
            "error": "Direct mode currently only supports single server tasks",
            "iterations": 0,
            "token_usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
            "execution_log": execution_log,
        }

    # Load server configuration
    try:
        config = load_server_config(server_names[0])
    except Exception as e:
        return {
            "task_id": task_id,
            "success": False,
            "execution_time_ms": int((time.time() - start_time) * 1000),
            "output": "",
            "error": f"Failed to load config for server '{server_names[0]}': {e}",
            "iterations": 0,
            "token_usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
            "execution_log": execution_log,
        }

    server_params = StdioServerParameters(
        command=config["command"],
        args=config["args"],
        env=config["env"],
        cwd=config["cwd"],
    )

    # Connect to MCP server using proper async context manager
    async with stdio_client(server_params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()

            if verbose:
                print(f"✓ Connected to {config['name']} MCP server")

            # List all available tools
            tools_result = await session.list_tools()
            server_name = config['name']

            if verbose:
                print(f"  - {server_name}: {len(tools_result.tools)} tools available")

            # Create Langchain tools that wrap MCP tool calls
            langchain_tools = []

            for tool_info in tools_result.tools:
                tool_name = f"{server_name}_{tool_info.name}"

                # Create a closure to capture tool_info and session
                def make_tool_func(srv_name, tl_info, sess):
                    async def tool_func(**kwargs) -> str:
                        """Call an MCP tool directly."""
                        tool_start = time.time()
                        result = await sess.call_tool(tl_info.name, kwargs)
                        tool_time = int((time.time() - tool_start) * 1000)

                        execution_log["tool_calls"].append({
                            "server": srv_name,
                            "tool": tl_info.name,
                            "args": kwargs,
                            "execution_time_ms": tool_time,
                        })

                        # Extract text content from result
                        if result.content:
                            if isinstance(result.content, list):
                                return "\n".join(str(c.text if hasattr(c, 'text') else c) for c in result.content)
                            return str(result.content)
                        return ""

                    return tool_func

                # Create structured tool
                tool = StructuredTool.from_function(
                    coroutine=make_tool_func(server_name, tool_info, session),
                    name=tool_name,
                    description=tool_info.description or f"{server_name}.{tool_info.name}",
                )
                langchain_tools.append(tool)

            if verbose:
                print(f"\n✓ Created {len(langchain_tools)} Langchain tool wrappers")

            # Create LLM with tool binding
            llm = ChatOpenAI(
                model=model_name,
                openai_api_key=openrouter_key,
                openai_api_base="https://openrouter.ai/api/v1",
                temperature=0,
            ).bind_tools(langchain_tools)

            # Run agentic loop
            try:
                messages = [
                    SystemMessage(
                        content=f"""You are an autonomous assistant that completes benchmark tasks using MCP tools.

CRITICAL INSTRUCTIONS:
- Work AUTONOMOUSLY - do NOT ask for permission or confirmation
- Execute all steps immediately without waiting for user approval
- Follow the task description exactly as specified
- Complete the entire task in one session

AVAILABLE TOOLS:
You have direct access to MCP server tools. Tool names are prefixed with the server name.
For example: Wikipedia_searchWikipedia, Wikipedia_getArticle, etc.

WORKFLOW:
1. Call the appropriate tools to complete each step of the task
2. Use the results from one tool call to inform the next
3. Continue until the task is fully completed

IMPORTANT:
- Never ask "Would you like me to proceed?" - just proceed
- Never ask "Should I continue?" - just continue
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

                    # Track token usage
                    if hasattr(response, "response_metadata"):
                        metadata = response.response_metadata
                        if "token_usage" in metadata:
                            token_usage = metadata["token_usage"]
                            total_input_tokens += token_usage.get("prompt_tokens", 0)
                            total_output_tokens += token_usage.get("completion_tokens", 0)

                    messages.append(response)

                    # Check if done (no tool calls)
                    if not response.tool_calls:
                        final_output = response.content
                        execution_log["timing"][f"iteration_{iteration}"] = iteration_time
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

                        # Find and execute the tool
                        tool_result = None
                        for tool in langchain_tools:
                            if tool.name == tool_name:
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
                    "token_usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                    "execution_log": execution_log,
                }


async def main():
    parser = argparse.ArgumentParser(
        description="Run MCP-Bench WITHOUT pctx (direct MCP server connections)"
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

    print(f"Running task: {task['task_id']}")
    print(f"Servers: {', '.join(task['servers'])}")
    print(f"Model: {args.model}")
    print(f"Mode: DIRECT (without pctx)\n")

    # Run task
    result = await run_task(
        task=task,
        model_name=args.model,
        openrouter_key=args.openrouter_key,
        verbose=args.verbose,
    )

    # Save detailed results
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    results_dir = Path(__file__).parent / "results" / f"{timestamp}_direct"
    results_dir.mkdir(parents=True, exist_ok=True)

    # Save full result
    result_file = results_dir / f"{task['task_id']}.json"

    with open(result_file, "w") as f:
        json.dump(
            {
                "task_id": task["task_id"],
                "task_description": task["task_description"],
                "servers": task["servers"],
                "model": args.model,
                "mode": "direct",
                "timestamp": timestamp,
                "result": result,
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
        # Group by tool name
        tool_counts = {}
        for call in tool_calls:
            tool_name = call["tool"]
            tool_counts[tool_name] = tool_counts.get(tool_name, 0) + 1
        for tool_name, count in sorted(tool_counts.items()):
            print(f"  - {tool_name}: {count}")

    if result["output"]:
        print(f"\nOutput: {result['output'][:300]}")
    if result["error"]:
        print(f"\nError: {result['error']}")

    print(f"\n{'=' * 60}")
    print(f"Results saved to: {results_dir}")
    print(f"  - Task result: {result_file.name}")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    asyncio.run(main())
