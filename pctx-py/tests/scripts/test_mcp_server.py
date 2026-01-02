#!/usr/bin/env python3
"""
Simple test MCP server for integration testing.

This server provides a few basic tools that can be used to test
stdio MCP server registration and function calling.
"""

from mcp.server.fastmcp import FastMCP

# Create an MCP server
mcp = FastMCP("TestMcpServer")


@mcp.tool()
def add(a: int, b: int) -> int:
    """Add two numbers together"""
    return a + b


@mcp.tool()
def multiply(x: int, y: int) -> int:
    """Multiply two numbers together"""
    return x * y


@mcp.tool()
def greet(name: str, greeting: str = "Hello") -> str:
    """Generate a greeting message"""
    return f"{greeting}, {name}!"


@mcp.tool()
def echo(message: str) -> dict:
    """Echo back a message with metadata"""
    return {
        "original": message,
        "length": len(message),
        "reversed": message[::-1],
    }


# Run with stdio transport for testing
if __name__ == "__main__":
    mcp.run(transport="stdio")
