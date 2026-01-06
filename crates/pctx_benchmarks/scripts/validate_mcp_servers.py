#!/usr/bin/env python3
"""
Validate that all configured MCP servers can be started and respond correctly.

This script tests each MCP server by:
1. Starting the server via pctx
2. Calling list_functions to verify it responds
3. Reporting success/failure for each server
"""

import asyncio
import json
import os
import sys
from pathlib import Path
from typing import Any

try:
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client
except ImportError as e:
    print(f"Error: {e}")
    print("Install: pip install mcp")
    sys.exit(1)


def load_env_file():
    """Load environment variables from .env file."""
    env_file = Path(__file__).parent.parent / ".env"
    if not env_file.exists():
        print("Warning: .env file not found, API keys may not be available")
        return

    with open(env_file) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#"):
                if "=" in line:
                    key, value = line.split("=", 1)
                    os.environ[key] = value
    print(f"✓ Loaded environment variables from .env\n")


def load_mcp_server_configs() -> dict:
    """Load MCP server configs from mcp_servers.json."""
    config_file = Path(__file__).parent.parent / "mcp_servers.json"
    if not config_file.exists():
        print(f"Error: {config_file} not found")
        sys.exit(1)

    with open(config_file) as f:
        data = json.load(f)
        return data.get("servers", {})


def substitute_env_vars(server_config: dict) -> dict:
    """Substitute ${VAR_NAME} with actual environment variables."""
    server_copy = server_config.copy()
    if "env" in server_copy and server_copy["env"]:
        env_substituted = {}
        for key, value in server_copy["env"].items():
            if isinstance(value, str) and value.startswith("${") and value.endswith("}"):
                env_var_name = value[2:-1]
                env_value = os.environ.get(env_var_name, "")
                if not env_value:
                    print(
                        f"  ⚠️  Warning: Environment variable {env_var_name} not set"
                    )
                env_substituted[key] = env_value
            else:
                env_substituted[key] = value
        server_copy["env"] = env_substituted
    return server_copy


async def validate_server(server_name: str, server_config: dict) -> dict[str, Any]:
    """Validate a single MCP server by starting it and calling list_functions."""
    print(f"Testing {server_name}...", end=" ", flush=True)

    # Substitute environment variables
    server_with_env = substitute_env_vars(server_config)

    # Create temporary pctx config for this server
    pctx_config = {
        "name": "pctx-validation",
        "version": "0.1.0",
        "servers": [server_with_env],
    }

    config_path = Path(f"/tmp/pctx_validate_{server_name.replace(' ', '_')}.json")
    with open(config_path, "w") as f:
        json.dump(pctx_config, f, indent=2)

    try:
        # Start pctx MCP server
        server_params = StdioServerParameters(
            command="pctx",
            args=["mcp", "start", "--stdio", "--config", str(config_path)],
            env=None,
        )

        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                # Initialize session
                await session.initialize()

                # Call list_functions to verify server responds
                result = await session.call_tool("list_functions", {})

                # Check if we got a valid response
                if result.content and len(result.content) > 0:
                    response_text = result.content[0].text
                    # Basic validation - any non-empty response is considered valid
                    # since different servers return different formats
                    if response_text and len(response_text) > 0:
                        print("✓ PASS")
                        return {
                            "server": server_name,
                            "status": "success",
                            "error": None,
                            "response_length": len(response_text),
                        }
                    else:
                        print("✗ FAIL (empty response)")
                        return {
                            "server": server_name,
                            "status": "failed",
                            "error": "Empty response from list_functions",
                            "response": None,
                        }
                else:
                    print("✗ FAIL (no content)")
                    return {
                        "server": server_name,
                        "status": "failed",
                        "error": "No content in response from list_functions",
                        "response": None,
                    }

    except Exception as e:
        print(f"✗ FAIL ({type(e).__name__})")
        return {
            "server": server_name,
            "status": "failed",
            "error": str(e),
            "error_type": type(e).__name__,
        }
    finally:
        # Cleanup temp config
        if config_path.exists():
            config_path.unlink()


async def main():
    print("=" * 60)
    print("MCP Server Validation")
    print("=" * 60)
    print()

    # Load environment variables
    load_env_file()

    # Load server configs
    server_configs = load_mcp_server_configs()
    print(f"Found {len(server_configs)} MCP servers to validate\n")

    # Validate each server
    results = []
    for server_name, server_config in server_configs.items():
        result = await validate_server(server_name, server_config)
        results.append(result)

    # Print summary
    print()
    print("=" * 60)
    print("Validation Summary")
    print("=" * 60)

    passed = [r for r in results if r["status"] == "success"]
    failed = [r for r in results if r["status"] == "failed"]

    print(f"\nPassed: {len(passed)}/{len(results)}")
    print(f"Failed: {len(failed)}/{len(results)}")

    if failed:
        print("\nFailed servers:")
        for result in failed:
            print(f"  ✗ {result['server']}")
            print(f"    Error: {result['error']}")

    print("\n" + "=" * 60)

    # Exit with error code if any failed
    sys.exit(0 if len(failed) == 0 else 1)


if __name__ == "__main__":
    asyncio.run(main())
