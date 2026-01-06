#!/usr/bin/env -S uv run --script
# /// script
# dependencies = [
#   "mcp>=0.1.0",
# ]
# ///
"""Test script to see what list_functions returns from pctx."""

import asyncio
import json
from pathlib import Path

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client


async def test_list_functions():
    """Test what list_functions returns."""
    config_path = Path("/tmp/pctx_benchmark.json")

    server_params = StdioServerParameters(
        command="pctx",
        args=["mcp", "start", "--stdio", "--config", str(config_path)],
        env=None,
    )

    print("Starting pctx MCP server...")
    async with stdio_client(server_params) as (read, write):
        async with ClientSession(read, write) as session:
            print("Initializing session...")
            await session.initialize()

            print("\nCalling list_functions...")
            result = await session.call_tool("list_functions", {})

            print("\n" + "="*60)
            print("list_functions result:")
            print("="*60)
            if result.content:
                for content in result.content:
                    if hasattr(content, 'text'):
                        print(content.text)
                    else:
                        print(content)
            else:
                print("No content returned")
            print("="*60)


if __name__ == "__main__":
    asyncio.run(test_list_functions())
