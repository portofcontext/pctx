import asyncio
import pprint
from time import sleep

from pctx_client import Pctx, tool

# @tool
# def now_timestamp() -> float:
#     """Returns current timestamp"""
#     return datetime.now().timestamp()


# @tool
# def search_logs(query: str = "", level: str = "info", limit: int = 100) -> list[dict]:
#     """Search application logs with optional filters"""
#     return [
#         {"message": f"match for '{query}'", "level": level, "index": i}
#         for i in range(min(limit, 3))
#     ]


# @tool("add", namespace="my_math")
# def add(a: float, b: float) -> float:
#     """adds two numbers"""
#     return a + b


# @tool("subtract", namespace="my_math")
# def subtract(a: float, b: float) -> float:
#     """subtracts b from a"""
#     return a - b


# class MultiplyOutput(BaseModel):
#     message: str
#     result: float


# @tool("multiply", namespace="my_math")
# def multiply(a: float, b: float) -> MultiplyOutput:
#     """multiplies a and b"""
#     return MultiplyOutput(message=f"Show your work! {a} * {b} = {a * b}", result=a * b)


@tool
def get_weather(city: str) -> str:
    """Get the current weather for a city."""
    return f"72°F and sunny in {city}"


@tool
def get_time(timezone: str) -> str:
    """Get the current time in a given timezone."""
    print("SLEEPING!")
    sleep(15)
    return f"3:00 PM in {timezone}"


async def main():
    async with Pctx(
        # url="https://....",
        # api_key="pctx_xxxx",
        # tools=[add, subtract, multiply, now_timestamp, search_logs],
        tools=[get_time, get_weather],
        # servers=[
        #     {
        #         "name": "stripe",
        #         "url": "https://mcp.stripe.com",
        #         "auth": {
        #             "type": "bearer",
        #             "token": getenv("STRIPE_MCP_KEY"),
        #         },
        #     }
        # ],
    ) as p:
        # print("+++++++++++ LIST +++++++++++\n")
        # print((await p.list_functions()).code)

        # print("\n\n+++++++++++ DETAILS +++++++++++\n")
        # print((await p.get_function_details(["MyMath.add", "Tools.nowTimestamp"])).code)

        code = """
async function run() {
    // Get weather for London
    const weather = await Tools.getWeather({ city: "London" });

    // Get current time in London timezone
    const time = await Tools.getTime({ timezone: "Europe/London" });

    return {
        weather,
        time
    };
}
    """
        output = await p.execute(code)
        pprint.pprint(output)


#         invalid_code = """
# async function run() {
#     let addval = await MyMath.add({a: "40", b: 2}); // invalid because `a` must be a number

#     return addval;
# }
#     """
#         invalid_output = await p.execute(invalid_code)
#         pprint.pprint(invalid_output)

#         print(p._session_id)


if __name__ == "__main__":
    asyncio.run(main())
