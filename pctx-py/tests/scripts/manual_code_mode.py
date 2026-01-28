import asyncio
import pprint
from datetime import datetime
from os import getenv

from pydantic import BaseModel

from pctx_client import Pctx, tool


@tool
def now_timestamp() -> float:
    """Returns current timestamp"""
    return datetime.now().timestamp()


@tool("add", namespace="my_math")
def add(a: float, b: float) -> float:
    """adds two numbers"""
    return a + b


@tool("subtract", namespace="my_math")
def subtract(a: float, b: float) -> float:
    """subtracts b from a"""
    return a - b


class MultiplyOutput(BaseModel):
    message: str
    result: float


@tool("multiply", namespace="my_math")
def multiply(a: float, b: float) -> MultiplyOutput:
    """multiplies a and b"""
    return MultiplyOutput(message=f"Show your work! {a} * {b} = {a * b}", result=a * b)


async def main():
    async with Pctx(
        # url="https://....",
        # api_key="pctx_xxxx",
        tools=[add, subtract, multiply, now_timestamp],
        servers=[
            {
                "name": "stripe",
                "url": "https://mcp.stripe.com",
                "auth": {
                    "type": "bearer",
                    "token": getenv("STRIPE_MCP_KEY"),
                },
            }
        ],
    ) as p:
        print("+++++++++++ LIST +++++++++++\n")
        print((await p.list_functions()).code)

        print("\n\n+++++++++++ DETAILS +++++++++++\n")
        print((await p.get_function_details(["MyMath.add", "Tools.nowTimestamp"])).code)

        code = """
async function run() {
    let addval = await MyMath.add({a: 40, b: 2});
    let subval = await MyMath.subtract({a: addval, b: 2});
    let multval = await MyMath.multiply({a: subval, b: 2});
    let now = await Tools.nowTimestamp({});
    let customers = await Stripe.listCustomers({});


    return { multval, now };
}
    """
        output = await p.execute(code)
        pprint.pprint(output)

        invalid_code = """
async function run() {
    let addval = await MyMath.add({a: "40", b: 2}); // invalid because `a` must be a number

    return addval;
}
    """
        invalid_output = await p.execute(invalid_code)
        pprint.pprint(invalid_output)

        print(p._session_id)


if __name__ == "__main__":
    asyncio.run(main())
