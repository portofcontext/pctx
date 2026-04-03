import asyncio

from claude_agent_sdk import ClaudeAgentOptions, create_sdk_mcp_server, query
from rich import print

from pctx_client import Pctx, tool


@tool
def get_weather(city: str) -> str:
    """Get weather for a given city."""
    return f"It's always sunny in {city}!"


@tool
def get_time(city: str) -> str:
    """Get time for a given city."""
    return f"It is midnight in {city}!"


async def run_agent():
    async with Pctx(tools=[get_weather, get_time]) as p:
        claude_tools = p.claude_agent_sdk_tools()
        mcp = create_sdk_mcp_server(name="weather + time codemode", tools=claude_tools)
        print([f"mcp__tools__{t.name}" for t in claude_tools])
        async for message in query(
            prompt="You are a helpful assistant, use tools when you need to access real-time information, you must use the tools mcp not websearch. What is the weather & time in SF?",
            options=ClaudeAgentOptions(
                mcp_servers={"tools": mcp},
                allowed_tools=[f"mcp__tools__{t.name}" for t in claude_tools]
            ),
        ):
            print(message)


if __name__ == "__main__":

    asyncio.run(run_agent())
