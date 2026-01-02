<div align="center">
  <img src="../.github/assets/logo.png" alt="PCTX Logo" style="height: 128px">
  <h1>Python pctx-client</h1>

[![Made by](https://img.shields.io/badge/MADE%20BY-Port%20of%20Context-1e40af.svg?style=for-the-badge&labelColor=0c4a6e)](https://portofcontext.com)

[![Python](https://img.shields.io/pypi/v/pctx-client?color=blue)](https://pctx.readthedocs.io/en/latest/)
[![Docs](https://img.shields.io/readthedocs/pctx)](https://pctx.readthedocs.io/en/latest/)

</div>

<div align="center">

Python client for using Code Mode via pctx - allow agents to execute code with your custom tools and MCP servers.

This README contains the quickstart, guides, and concept overviews. See
[Python API Reference](https://pctx.readthedocs.io/en/latest/) for full reference documentation.

</div>

## Installation

```bash
pip install pctx-client
```

## Quick Start

1. Install PCTX server

```bash
# Homebrew
brew install portofcontext/tap/pctx

# cURL
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/portofcontext/pctx/main/install.sh | sh

# npm
npm i -g @portofcontext/pctx
```

2. Install Python pctx client with the langchain extra & additional langchain dependencies. (pctx supports other agent frameworks as well, see [Agent Frameworks](#agent-frameworks))

```
pip install pctx-client[langchain] langchain langchain_openai
```

3. Set the OpenRouter API key ([create an account to get a key](https://openrouter.ai/))

```bash
export OPENROUTER_API_KEY=*****
```

3. Start the Code Mode server

```bash
pctx start
```

4. Define and run `main.py`

```python
import asyncio
import pprint
import os

from pctx_client import Pctx, tool
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI

# Define your tools
@tool
def get_weather(city: str) -> str:
    """Get weather for a given city."""
    return f"It's always sunny in {city}!"


@tool
def get_time(city: str) -> str:
    """Get time for a given city."""
    return f"It is midnight in {city}!"


async def main(api_key: str):
    # Initialize pctx client with your tools
    p = Pctx(tools=[get_weather, get_time])

    # Define your agent
    llm = ChatOpenAI(
        model="deepseek/deepseek-chat",
        temperature=0,
        api_key=api_key,
        base_url="https://openrouter.ai/api/v1",
        max_retries=2,
    )
    agent = create_agent(
        llm,
        tools=p.langchain_tools(),
        system_prompt="You are a helpful assistant",
    )

    # Connect to pctx
    await p.connect()

    result = await agent.ainvoke(
        {
            "messages": [
                {"role": "user", "content": "what is the weather and time in nyc"}
            ]
        }
    )

    pprint.pprint(result)

    # Disconnect when done
    await p.disconnect()

if __name__ == "__main__":
    api_key = os.getenv("OPENROUTER_API_KEY")
    if api_key is None:
        raise EnvironmentError(
            "OPENROUTER_API_KEY not set in the environment. "
            "Get your API key from https://openrouter.ai/settings/keys"
        )

    asyncio.run(run(api_key))
```

## Code Mode

Code Mode allows AI agents to execute TypeScript code with access to both your custom Python tools and MCP servers. Instead of requiring separate tool calls for each operation, agents can write and execute code that orchestrates multiple function calls, processes data, and returns results - all in a single execution.

The `Pctx` client provides 3 main code mode functions:

1. **`list_functions()`** - Lists all available functions organized by namespace. LLMs are instructed to call this first to discover what functions are available from your registered tools and MCP servers.

2. **`get_function_details(functions)`** - Returns detailed information about specific functions including parameter types, return values. LLMs are instructed to call this after `list_functions()` to understand the required/optional inputs and outputs of Code Mode functions.

3. **`execute(code)`** - Executes TypeScript code in an isolated Deno sandbox. The code can call any namespaced functions (e.g., `Namespace.functionName()`) discovered via `list_functions()`. Returns the execution result with stdout, stderr, and return value.

## Defining Tools

pctx provides two approaches for defining tools: the `@tool` decorator for simple function-based tools, and `Tool`/`AsyncTool` classes for more complex implementations.

### Decorator Approach

The `@tool` decorator is the simplest way to create tools from functions. It automatically extracts type hints and docstrings to create the tool schema.

#### Basic Example

```python
from pctx_client import tool

@tool
def get_weather(city: str) -> str:
    """Get weather information for a given city."""
    return f"It's always sunny in {city}!"


pctx = Pctx(tools=[get_weather])
```

#### Custom Name and Namespace

```python
@tool(
    name="weather_lookup",
    namespace="weather_api",
    description="Fetches current weather conditions for any city"
)
def fetch_weather(location: str) -> str:
    return f"Weather for {location}: Sunny, 72°F"


pctx = Pctx(tools=[fetch_weather])
```

#### Async Tools

```python
import asyncio

@tool
async def fetch_user_data(user_id: int) -> dict[str, str]:
    """Asynchronously fetch user data from an API."""
    await asyncio.sleep(0.1)  # Simulate API call
    return {"id": str(user_id), "name": "John Doe"}


pctx = Pctx(tools=[fetch_user_data])
```

#### Nested Types with Pydantic

```python
from pydantic import BaseModel, Field
from typing import List, Optional

class Address(BaseModel):
    street: str
    city: str
    zip_code: str = Field(description="5-digit ZIP code")
    country: str = "USA"

class UserProfile(BaseModel):
    name: str
    age: int
    email: str
    addresses: List[Address]
    preferences: Optional[dict[str, bool]] = None

class UpdateResult(BaseModel):
    success: bool
    user_id: str
    updated_fields: List[str]
    message: str

@tool
def update_user_profile(
    user_id: str,
    profile: UserProfile,
    notify: bool = True
) -> UpdateResult:
    """
    Update a user's profile with complex nested data.

    This tool demonstrates handling of complex Pydantic models with
    nested objects, lists, and optional fields.
    """
    # Process the update
    updated_fields = ["name", "age", "email", "addresses"]

    return UpdateResult(
        success=True,
        user_id=user_id,
        updated_fields=updated_fields,
        message=f"Successfully updated profile for user {user_id}"
    )


pctx = Pctx(tools=[update_user_profile])
```

### Class-Based Approach

For more control over tool behavior and state, you can subclass `Tool` (synchronous) or `AsyncTool` (asynchronous) and implement the `_invoke` or `_ainvoke` method. When implementing the class based approach you **MUST** define the `input_schema` and `output_schema` attributes to match the `_invoke` or `_ainvoke` method implementation

#### Synchronous Tool Class

```python
from pctx_client import Tool
from pydantic import BaseModel
from typing import Any, Literal

class CalculatorInput(BaseModel):
    operation: Literal["add", "subtract", "multiply", "divide"]
    x: float
    y: float

class Calculator(Tool):
    name: str = "calculator"
    namespace: str = "math"
    description: str = "Performs basic arithmetic operations"
    input_schema: type[BaseModel] = CalculatorInput
    output_schema: type[float] = float

    def _invoke(
        self,
        operation: Literal["add", "subtract", "multiply", "divide"],
        x: float,
        y: float,
    ) -> float:
        """Execute the calculation based on the operation."""
        if operation == "add":
            return x + y
        elif operation == "subtract":
            return x - y
        elif operation == "multiply":
            return x * y
        elif operation == "divide":
            return x / y
        else:
            raise ValueError(f"Unknown operation: {operation}")



pctx = Pctx(tools=[Calculator()])
```

#### Asynchronous Tool Class

```python
from pctx_client import AsyncTool
from pydantic import BaseModel, Field
import httpx
from typing import List

class SearchQuery(BaseModel):
    query: str = Field(description="The search term")
    max_results: int = Field(default=10, description="Maximum results to return")
    filters: dict[str, str] = Field(default_factory=dict)

class SearchResult(BaseModel):
    title: str
    url: str
    snippet: str
    score: float

class SearchResponse(BaseModel):
    results: List[SearchResult]
    total_count: int
    query_time_ms: float

class WebSearchTool(AsyncTool):
    name: str = "web_search"
    namespace: str = "search"
    description: str = "Search the web and return relevant results"
    input_schema: type[BaseModel] = SearchQuery
    output_schema: type[SearchResponse] = SearchResponse

    async def _ainvoke(
        self,
        query: str,
        max_results: int = 10,
        filters: dict[str, str] = {}
    ) -> SearchResponse:
        """Perform an asynchronous web search."""
        # Simulate async API call
        async with httpx.AsyncClient() as client:
            # Mock implementation
            results = [
                SearchResult(
                    title=f"Result {i} for '{query}'",
                    url=f"https://example.com/result{i}",
                    snippet=f"This is a snippet for result {i}",
                    score=0.9 - (i * 0.1)
                )
                for i in range(1, min(max_results, 5) + 1)
            ]

            return SearchResponse(
                results=results,
                total_count=len(results),
                query_time_ms=45.2
            )


pctx = Pctx(tools=[WebSearchTool()])
```

#### Stateful Tool with Initialization

```python
from pctx_client import Tool
from pydantic import BaseModel
from typing import List

class QueryInput(BaseModel):
    sql: str
    params: dict[str, Any] = {}

class DatabaseTool(Tool):
    name: str = "database_query"
    namespace: str = "db"
    description: str = "Execute SQL queries against the database"
    input_schema: type[BaseModel] = QueryInput
    output_schema: type[List[dict]] = List[dict]

    # Custom fields for state
    connection_string: str
    max_rows: int = 1000

    def __init__(self, connection_string: str, **kwargs):
        super().__init__(connection_string=connection_string, **kwargs)
        # Initialize database connection
        self._setup_connection()

    def _setup_connection(self):
        """Set up database connection (mock)."""
        print(f"Connected to database: {self.connection_string}")

    def _invoke(self, sql: str, params: dict[str, Any] = {}) -> List[dict]:
        """Execute the SQL query."""
        # Mock database query
        return [
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ]


pctx = Pctx(
    tools=[
        DatabaseTool(connection_string="postgresql://localhost/mydb"),
    ],
)
```

### Registering Tools with pctx

Once you've defined your tools, register them with the `Pctx` client:

```python
from pctx_client import Pctx

# Register decorator-based tools
p = Pctx(tools=[get_weather, update_user_profile, fetch_user_data])

# Register class-based tools (pass instances)
calc = Calculator()
search = WebSearchTool()
db = DatabaseTool(connection_string="postgresql://localhost/mydb")

p = Pctx(tools=[calc, search, db])

# Mix both approaches
p = Pctx(tools=[get_weather, calc, search, fetch_user_data])
```

## Registering MCP Servers

pctx supports connecting to MCP servers to extend your agent's capabilities. You can register both HTTP-based and stdio-based MCP servers.

### HTTP MCP Servers

```python
from pctx_client import Pctx

# HTTP server without authentication
servers = [
    {
        "name": "weather",
        "url": "http://localhost:3000/mcp"
    }
]

# HTTP server with bearer token authentication
servers = [
    {
        "name": "api",
        "url": "https://api.example.com/mcp",
        "auth": {
            "type": "bearer",
            "token": "your-api-token"
        }
    }
]

# HTTP server with custom headers authentication
servers = [
    {
        "name": "api",
        "url": "https://api.example.com/mcp",
        "auth": {
            "type": "headers",
            "headers": {
                "X-API-Key": "your-api-key",
                "X-Custom-Header": "custom-value"
            }
        }
    }
]

p = Pctx(servers=servers)
```

### Stdio MCP Servers

Stdio MCP servers communicate via stdin/stdout, making them ideal for local integrations and command-line tools.
NOTE: The stdio mcp servers must be running on the same host as pctx, which is not necessarily the same host as this python client.

```python
from pctx_client import Pctx

# Basic stdio server
servers = [
    {
        "name": "local-mcp",
        "command": "node"
    }
]

# Stdio server with arguments
servers = [
    {
        "name": "local-mcp",
        "command": "node",
        "args": ["./mcp-server.js", "--config", "config.json"]
    }
]

# Stdio server with environment variables
servers = [
    {
        "name": "local-mcp",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-everything"],
        "env": {
            "NODE_ENV": "production",
            "LOG_LEVEL": "info"
        }
    }
]

p = Pctx(servers=servers)
```

### Combining Tools and Servers

```python
from pctx_client import Pctx, tool

@tool
def custom_function(input: str) -> str:
    """A custom local function."""
    return f"Processed: {input}"

servers = [
    {"name": "api", "url": "https://api.example.com/mcp"},
    {"name": "local", "command": "node", "args": ["./server.js"]}
]

# Initialize with both local tools and MCP servers
p = Pctx(tools=[custom_function], servers=servers)
```

## Agent Frameworks

- `langchain`: Export pctx's Code Mode tools as [LangChain tools](https://docs.langchain.com/oss/python/langchain/tools)

```bash
pip install pctx-client[langchain]
```

- `crewai`: Export pctx's Code Mode tools as [CrewAI tools](https://docs.crewai.com/en/concepts/tools)

```bash
pip install pctx-client[crewai]
```

- `openai`: Export pctx's Code Mode tools as [OpenAI function tools](https://openai.github.io/openai-agents-python/tools/#function-tools)

```bash
pip install pctx-client[openai]
```

- `pydantic-ai`: Export pctx's Code Mode tools as [Pydantic Ai function tools](https://ai.pydantic.dev/tools/)

```bash
pip install pctx-client[pydantic-ai]
```

**pctx can easily be integrated into any agent framework by wrapping the 3 Code Mode tools available on the `Pctx` class with the frameworks tools, see [`Pctx().langchain_tools()`](./src/pctx_client/_client.py) for the langchain implementation**
