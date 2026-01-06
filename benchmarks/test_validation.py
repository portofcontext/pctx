#!/usr/bin/env -S uv run --script
# /// script
# dependencies = [
#   "mcp>=0.1.0",
# ]
# ///
"""Test validation of MCP server startup."""

import asyncio
from pathlib import Path

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client


async def test_validation():
    """Test that validation catches failed servers."""
    config_path = Path("/tmp/test_broken_config.json")

    server_params = StdioServerParameters(
        command="pctx",
        args=["mcp", "start", "--stdio", "--config", str(config_path)],
        env=None,
    )

    print("Testing with broken config...")
    try:
        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                await session.initialize()

                # Validate that MCP servers started successfully
                initial_check = await session.call_tool("list_functions", {})
                functions_list = initial_check.content[0].text if initial_check.content else ""

                print(f"\nFunctions list: '{functions_list}'")

                if not functions_list or functions_list.strip() == "":
                    print("✓ Validation correctly detected no functions available")
                    return True
                else:
                    print("✗ Validation failed - got functions when none expected")
                    return False

    except Exception as e:
        print(f"✓ Exception raised as expected: {e}")
        return True


if __name__ == "__main__":
    result = asyncio.run(test_validation())
    exit(0 if result else 1)
