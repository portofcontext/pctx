import asyncio
import os

from crewai import LLM, Agent

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
    code_mode = Pctx(tools=[get_weather, get_time])
    await code_mode.connect()

    llm = LLM(
        model="openai/gpt-oss-120b",
        api_key=os.environ.get("OPENROUTER_API_KEY"),
        base_url="https://openrouter.ai/api/v1",
    )

    agent = Agent(
        llm=llm,
        tools=code_mode.crewai_tools("filesystem"),
        verbose=True,
        role="Helpful assistant",
        goal="answer queries about time and weather with your available tools",
        backstory="you have no information other than what is returned by the tools, you MUST use your tools. Only write code when ready to call the execute tool",
    )
    await agent.kickoff_async("what is the weather and time in sf")

    await code_mode.disconnect()


if __name__ == "__main__":
    if "OPENROUTER_API_KEY" not in os.environ:
        raise EnvironmentError(
            "OPENROUTER_API_KEY not set in the environment. "
            "Get your API key from https://openrouter.ai/settings/keys"
        )

    asyncio.run(run_agent())
