#!/usr/bin/env python3
"""
Run MCP-Bench benchmarks using pctx with OpenRouter.

Usage:
    python scripts/run_mcp_bench.py --openrouter-key YOUR_KEY
    python scripts/run_mcp_bench.py --model deepseek/deepseek-chat
"""

import argparse
import asyncio
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

from pctx_client import Pctx
from langchain_openai import ChatOpenAI
from langchain_core.messages import SystemMessage


def load_tasks(dataset_path: Path) -> list[dict[str, Any]]:
    """Load MCP-Bench tasks from JSON file."""
    with open(dataset_path) as f:
        data = json.load(f)

    # Flatten tasks from server_tasks structure, preserving server info
    all_tasks = []
    for server_entry in data.get("server_tasks", []):
        servers = server_entry.get("servers", [])
        for task in server_entry.get("tasks", []):
            task["servers"] = servers  # Add required servers to each task
            all_tasks.append(task)

    return all_tasks


# MCP server configurations (simplified - using publicly available packages)
MCP_SERVER_CONFIGS = {
    "OpenAPI Explorer": {
        "command": "npx",
        "args": ["-y", "openapi-mcp-server"],
        "env": {},
    },
    "Unit Converter": {
        "command": "npx",
        "args": ["-y", "@ivo-toby/unit-converter-mcp"],
        "env": {},
    },
    "Wikipedia": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-wikipedia"],
        "env": {},
    },
    "Google Maps": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-google-maps"],
        "env": {"GOOGLE_MAPS_API_KEY": os.environ.get("GOOGLE_MAPS_API_KEY", "")},
    },
}


async def run_task(pctx: Pctx, llm: ChatOpenAI, task: dict[str, Any]) -> dict[str, Any]:
    """Execute a single MCP-Bench task."""
    print(f"\n{'=' * 60}")
    print(f"Task: {task['task_id']}")
    print(f"Description: {task['task_description']}")
    print(f"{'=' * 60}")

    start_time = time.time()

    try:
        # Get available functions
        functions_code = await pctx.list_functions()

        # Simple approach: Ask LLM to write TypeScript code to complete the task
        system_msg = SystemMessage(
            content=f"""You are an expert TypeScript programmer.

Available functions (as TypeScript):
{functions_code}

Task: {task["task_description"]}

Write TypeScript code that:
1. Uses the available functions to complete the task
2. Defines an async run() function as the entry point
3. Returns the final result

Only respond with the TypeScript code, nothing else."""
        )

        response = await llm.ainvoke([system_msg])
        code = response.content

        # Strip markdown code fences if present
        if isinstance(code, str):
            code = code.strip()
            if code.startswith("```"):
                # Remove opening fence (```typescript, ```ts, or just ```)
                lines = code.split("\n")
                if lines[0].startswith("```"):
                    lines = lines[1:]
                # Remove closing fence
                if lines and lines[-1].strip() == "```":
                    lines = lines[:-1]
                code = "\n".join(lines)

        print(f"\nGenerated code:\n{code}\n")

        # Execute the code
        result = await pctx.execute(code)

        execution_time_ms = int((time.time() - start_time) * 1000)

        success = result.success and not result.runtime_error

        return {
            "task_id": task["task_id"],
            "success": success,
            "execution_time_ms": execution_time_ms,
            "output": result.stdout if success else "",
            "error": result.runtime_error.message if result.runtime_error else None,
        }

    except Exception as e:
        execution_time_ms = int((time.time() - start_time) * 1000)
        print(f"❌ Error: {e}")
        return {
            "task_id": task["task_id"],
            "success": False,
            "execution_time_ms": execution_time_ms,
            "output": "",
            "error": str(e),
        }


async def main():
    parser = argparse.ArgumentParser(
        description="Run MCP-Bench with pctx and OpenRouter"
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
        "--dataset",
        default="data/mcpbench_tasks_single_runner_format.json",
        help="Path to MCP-Bench dataset JSON",
    )
    parser.add_argument(
        "--max-tasks",
        type=int,
        default=5,
        help="Maximum number of tasks to run (default: 5)",
    )
    parser.add_argument(
        "--output",
        default="data/benchmark_results.json",
        help="Output file for results",
    )
    args = parser.parse_args()

    if not args.openrouter_key:
        print("Error: OpenRouter API key required")
        print("Set via --openrouter-key or OPENROUTER_API_KEY environment variable")
        print("Get your key at: https://openrouter.ai/settings/keys")
        sys.exit(1)

    # Load tasks
    dataset_path = Path(__file__).parent.parent / args.dataset
    if not dataset_path.exists():
        print(f"Error: Dataset not found at {dataset_path}")
        print("Run: cargo run --bin benchmark download")
        sys.exit(1)

    tasks = load_tasks(dataset_path)
    print(f"Loaded {len(tasks)} tasks from {dataset_path}")

    # Limit tasks
    tasks = tasks[: args.max_tasks]
    print(f"Running {len(tasks)} tasks with model: {args.model}\n")

    # Initialize LLM
    llm = ChatOpenAI(
        model=args.model,
        temperature=0,
        api_key=args.openrouter_key,
        base_url="https://openrouter.ai/api/v1",
        max_retries=2,
    )

    # Run all tasks
    results = []
    for task in tasks:
        # Initialize pctx with MCP servers required for this task
        required_servers = task.get("servers", [])

        # Filter to only servers we have configs for
        # Note: pctx Python client expects MCP servers as command-line style configs
        # We need to check the actual Pctx() API to see how to pass MCP servers
        skip_task = False
        for server_name in required_servers:
            if server_name not in MCP_SERVER_CONFIGS:
                print(
                    f"⚠️  Warning: No config for MCP server '{server_name}', skipping task"
                )
                results.append(
                    {
                        "task_id": task["task_id"],
                        "success": False,
                        "execution_time_ms": 0,
                        "output": "",
                        "error": f"Missing MCP server config: {server_name}",
                    }
                )
                skip_task = True
                break

        if skip_task:
            continue

        # Build server configs for pctx
        # ServerConfig expects: {"name": str, "url": str}
        # For stdio MCP servers spawned via npx, the pctx server must handle spawning
        # This requires the pctx server to be configured with MCP servers in its config
        # For now, MCP servers must be pre-configured on the pctx server side
        print("⚠️  MCP server support requires server-side configuration - task may fail")
        pctx = Pctx(tools=[])

        try:
            await pctx.connect()
            result = await run_task(pctx, llm, task)
            results.append(result)

            status = "✓" if result["success"] else "✗"
            print(f"{status} {result['task_id']}: {result['execution_time_ms']}ms")
        except Exception as e:
            print(f"❌ Error running task {task['task_id']}: {e}")
            results.append(
                {
                    "task_id": task["task_id"],
                    "success": False,
                    "execution_time_ms": 0,
                    "output": "",
                    "error": str(e),
                }
            )
        finally:
            await pctx.disconnect()

    # Generate report
    total = len(results)
    successful = sum(1 for r in results if r["success"])
    failed = total - successful
    avg_time = sum(r["execution_time_ms"] for r in results) / total if total > 0 else 0

    report = {
        "model": args.model,
        "total_tasks": total,
        "successful_tasks": successful,
        "failed_tasks": failed,
        "success_rate": (successful / total * 100) if total > 0 else 0,
        "average_execution_time_ms": avg_time,
        "results": results,
    }

    # Print summary
    print(f"\n{'=' * 60}")
    print(f"MCP-Bench Results: {args.model}")
    print(f"{'=' * 60}")
    print(f"Total Tasks:     {total}")
    print(f"Successful:      {successful} ✓")
    print(f"Failed:          {failed} ✗")
    print(f"Success Rate:    {report['success_rate']:.1f}%")
    print(f"Avg Exec Time:   {avg_time:.0f} ms")
    print(f"{'=' * 60}")

    # Save results
    output_path = Path(__file__).parent.parent / args.output
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(report, f, indent=2)
    print(f"\nResults saved to: {output_path}")


if __name__ == "__main__":
    asyncio.run(main())
