"""Search and fetch through pctx's unified MCP server, without an LLM."""

import asyncio
import os
from pathlib import Path

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

CODE = """
async function run() {
  const search = await Parallel.webSearch({
    objective: "Find the official pctx documentation for upstream MCP servers.",
    search_queries: ["pctx upstream MCP servers documentation"]
  });
  const page = await Parallel.webFetch({
    urls: ["https://github.com/portofcontext/pctx"],
    objective: "Explain how pctx connects to upstream MCP servers.",
    session_id: search.session_id
  });
  return {
    search: search.results.slice(0, 3).map(result => ({
      title: result.title, url: result.url, excerpts: result.excerpts
    })),
    pages: page.results.map(result => ({
      url: result.url, excerpts: result.excerpts
    })),
    fetchErrors: page.errors
  };
}
"""


async def main():
    config = Path(__file__).with_name("pctx.json")
    params = StdioServerParameters(
        command="pctx",
        env=dict(os.environ),
        args=["--config", str(config), "mcp", "start", "--stdio", "--no-banner"],
    )
    async with stdio_client(params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()
            # Discover pctx's generated namespace and signatures before execution.
            for name, arguments in [
                ("list_functions", {}),
                (
                    "get_function_details",
                    {"functions": ["Parallel.webSearch", "Parallel.webFetch"]},
                ),
                ("execute_typescript", {"code": CODE}),
            ]:
                result = await session.call_tool(name, arguments)
                if result.isError:
                    raise RuntimeError(result.model_dump_json())
                print(f"\n{name}:")
                for content in result.content:
                    if content.type == "text":
                        print(content.text)


if __name__ == "__main__":
    asyncio.run(main())
