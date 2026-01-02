#!/usr/bin/env python3
"""
Simple HTTP MCP server for integration testing.

This server provides basic tools over HTTP to test HTTP MCP server
registration and function calling.
"""

from mcp.server.fastmcp import FastMCP
import uvicorn

# Create an MCP server
mcp = FastMCP("test_http_mcp")


@mcp.tool()
def subtract(a: int, b: int) -> int:
    """Subtract b from a"""
    return a - b


@mcp.tool()
def divide(x: int, y: int) -> float:
    """Divide x by y"""
    if y == 0:
        raise ValueError("Cannot divide by zero")
    return x / y


@mcp.tool()
def concat(first: str, second: str, separator: str = " ") -> str:
    """Concatenate two strings with a separator"""
    return f"{first}{separator}{second}"


@mcp.tool()
def reverse_string(text: str) -> dict:
    """Reverse a string and return metadata"""
    return {
        "original": text,
        "reversed": text[::-1],
        "length": len(text),
    }


# Run with streamable-http transport for HTTP testing
if __name__ == "__main__":
    # FastMCP's streamable-http transport is compatible with rmcp's StreamableHttpClientTransport
    app = mcp.streamable_http_app()
    uvicorn.run(app, host="127.0.0.1", port=8765, log_level="warning")
