"""
Example script demonstrating PCTX integration with Pydantic AI

This script shows how to use PCTX code mode tools with Pydantic AI.
Requires: pip install pctx[pydantic-ai]

Set the OPENROUTER_API_KEY environment variable before running.
"""

import asyncio
import os

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
    """Run a Pydantic AI agent with PCTX code mode tools"""
    try:
        from pydantic_ai import Agent
    except ImportError:
        print(
            "Error: pydantic-ai not installed. Install with: pip install pctx[pydantic-ai]"
        )
        return

    # Initialize PCTX with local tools
    code_mode = Pctx(tools=[get_weather, get_time])
    await code_mode.connect()

    # Get PCTX tools in Pydantic AI format
    pctx_tools = code_mode.pydantic_ai_tools("filesystem")

    # Create a Pydantic AI agent with PCTX tools
    agent = Agent(
        "openrouter:anthropic/claude-sonnet-4-5",
        system_prompt="You are a helpful assistant with access to code execution tools.",
        tools=pctx_tools,
    )

    print("Running Pydantic AI agent with PCTX tools...")

    # Run the agent
    result = await agent.run("What is the weather and time in San Francisco?")

    print(f"\nAgent Response:\n{result.output}")

    # Show tool calls if any were made
    if hasattr(result, "all_messages"):
        tool_calls = [
            msg
            for msg in result.all_messages()
            if hasattr(msg, "parts")
            and any(hasattr(part, "tool_name") for part in msg.parts)
        ]
        if tool_calls:
            print(f"\nTool calls made: {len(tool_calls)}")

    await code_mode.disconnect()


async def run_streaming_agent():
    """Example of streaming responses with Pydantic AI and PCTX"""
    try:
        from pydantic_ai import Agent
    except ImportError:
        print(
            "Error: pydantic-ai not installed. Install with: pip install pctx[pydantic-ai]"
        )
        return

    code_mode = Pctx(tools=[get_weather, get_time])
    await code_mode.connect()

    pctx_tools = code_mode.pydantic_ai_tools()

    agent = Agent(
        "openrouter:deepseek/deepseek-chat",
        system_prompt="You are a helpful assistant.",
        tools=pctx_tools,
    )

    print("\nStreaming Agent Example:")
    print("-" * 50)

    # Stream the response
    async with agent.run_stream(
        "List available functions and then tell me the weather in Tokyo"
    ) as result:
        async for message in result.stream_text():
            print(message, end="", flush=True)

    print("\n" + "-" * 50)

    await code_mode.disconnect()


if __name__ == "__main__":
    if "OPENROUTER_API_KEY" not in os.environ:
        raise EnvironmentError(
            "OPENROUTER_API_KEY not set in the environment. "
            "Get your API key from https://openrouter.ai/settings/keys"
        )

    # Run both examples
    asyncio.run(run_agent())
    print("\n" + "=" * 50 + "\n")
    asyncio.run(run_streaming_agent())
