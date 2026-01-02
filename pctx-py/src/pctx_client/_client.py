"""
PCTX Client

Main client for executing code with both MCP tools and local Python tools.
"""

from typing import TYPE_CHECKING
from urllib.parse import urlparse

from httpx import AsyncClient

from pctx_client._tool import AsyncTool, Tool
from pctx_client._websocket_client import WebSocketClient
from pctx_client.exceptions import ConnectionError, SessionError
from pctx_client.models import (
    ExecuteInput,
    ExecuteOutput,
    GetFunctionDetailsInput,
    GetFunctionDetailsOutput,
    ListFunctionsOutput,
    ServerConfig,
    ToolConfig,
)
from pydantic import BaseModel

if TYPE_CHECKING:
    try:
        from langchain_core.tools import BaseTool as LangchainBaseTool
        from crewai.tools import BaseTool as CrewAiBaseTool
        from pydantic_ai.tools import Tool as PydanticAITool
        from agents import FunctionTool
    except ImportError:
        pass


class Pctx:
    """
    PCTX Client

    Execute TypeScript/JavaScript code with access to both MCP tools and local Python tools.
    """

    def __init__(
        self,
        tools: list[Tool | AsyncTool] | None = None,
        servers: list[ServerConfig] | None = None,
        url: str = "http://localhost:8080",
        execute_timeout: float = 30.0,
    ):
        """
        Initialize the PCTX client.

        Args:
            tools: List of local Python tools to register
            servers: List of MCP servers to register. Each server can be either:
                - HTTP server: {"name": "...", "url": "...", "auth": {...}}
                - stdio server: {"name": "...", "command": "...", "args": [...], "env": {...}}
            url: PCTX server URL (default: http://localhost:8080)
            execute_timeout: Timeout for code execution in seconds (default: 30.0)
        """

        # Parse and normalize the URL
        parsed = urlparse(url)

        # Determine the base host and port
        if parsed.scheme in ["ws", "wss"]:
            # WebSocket URL provided - derive HTTP from it
            http_scheme = "https" if parsed.scheme == "wss" else "http"
            host = parsed.netloc
        elif parsed.scheme in ["http", "https"]:
            # HTTP URL provided - derive WebSocket from it
            http_scheme = parsed.scheme
            host = parsed.netloc
        else:
            raise ValueError(
                f"Invalid URL scheme: {parsed.scheme}. Expected http, https, ws, or wss"
            )

        ws_scheme = "wss" if http_scheme == "https" else "ws"

        self._ws_client = WebSocketClient(url=f"{ws_scheme}://{host}/ws", tools=tools)
        self._client = AsyncClient(base_url=f"{http_scheme}://{host}")
        self._session_id: str | None = None

        self._tools = tools or []
        self._servers = servers or []
        self._execute_timeout = execute_timeout

    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.disconnect()

    async def connect(self):
        """Creates CodeMode session, register local tools, and register MCP servers."""
        if self._session_id is not None:
            await self.disconnect()

        try:
            connect_res = await self._client.post("/code-mode/session/create")
            connect_res.raise_for_status()
        except Exception as e:
            # Check if this is a connection error (server not running)
            error_message = str(e).lower()
            if any(
                msg in error_message
                for msg in ["connection", "refused", "failed to connect", "unreachable"]
            ):
                raise ConnectionError(
                    f"Failed to connect to PCTX server at {self._client.base_url}. "
                    "Please ensure the server is running.\n"
                    "Start the server with: pctx server start"
                ) from e
            # Re-raise other errors as-is
            raise

        # Parse the session ID from the response
        try:
            self._session_id = connect_res.json()["session_id"]
        except (KeyError, ValueError) as e:
            raise ConnectionError(
                f"Received invalid response from PCTX server at {self._client.base_url}. "
                "The server may be running but not responding correctly."
            ) from e

        self._client.headers = {"x-code-mode-session": self._session_id or ""}

        # Register all local tools & MCP servers
        configs: list[ToolConfig] = [
            {
                "name": t.name,
                "namespace": t.namespace,
                "description": t.description,
                "input_schema": t.input_json_schema(),
                "output_schema": t.output_json_schema(),
            }
            for t in self._tools
        ]

        await self._register_tools(configs)
        await self._register_servers(self._servers)

    async def disconnect(self):
        """Disconnect closes current code-mode session."""
        close_res = await self._client.post("/code-mode/session/close")
        close_res.raise_for_status()
        self._session_id = None

    # ========== Main code mode methods method ==========

    async def list_functions(self) -> ListFunctionsOutput:
        """
        List all available functions organized by namespace.

        This is typically the first method you should call to discover what functions
        are available in the current session, including both registered local tools
        and MCP server functions.

        Returns:
            ListFunctionsOutput: An object containing function signatures organized
                by namespace. The `code` attribute contains TypeScript code with
                function declarations that can be used for reference.

        Raises:
            SessionError: If called before establishing a session via connect().

        Example:
            >>> async with Pctx() as pctx:
            ...     functions = await pctx.list_functions()
            ...     print(functions.code)  # TypeScript declarations
        """
        if self._session_id is None:
            raise SessionError(
                "No code mode session exists, run Pctx(...).connect() before calling"
            )
        list_res = await self._client.post("/code-mode/functions/list")
        list_res.raise_for_status()

        return ListFunctionsOutput.model_validate(list_res.json())

    async def get_function_details(
        self, functions: list[str]
    ) -> GetFunctionDetailsOutput:
        """
        Get detailed information about specific functions.

        After discovering available functions with list_functions(), use this method
        to get comprehensive details about parameter types, return values, and usage
        for the specific functions you need.

        Args:
            functions: List of function names in 'namespace.functionName' format
                (e.g., ['Notion.apiPostSearch', 'Weather.getCurrentWeather']).

        Returns:
            GetFunctionDetailsOutput: An object containing detailed TypeScript
                declarations for the requested functions. The `code` attribute
                contains the full function signatures with JSDoc comments.

        Raises:
            SessionError: If called before establishing a session via connect().

        Example:
            >>> async with Pctx() as pctx:
            ...     details = await pctx.get_function_details(['Weather.getCurrentWeather'])
            ...     print(details.code)  # Detailed TypeScript with parameter info
        """
        if self._session_id is None:
            raise SessionError(
                "No code mode session exists, run Pctx(...).connect() before calling"
            )
        list_res = await self._client.post(
            "/code-mode/functions/details", json={"functions": functions}
        )
        list_res.raise_for_status()

        return GetFunctionDetailsOutput.model_validate(list_res.json())

    async def execute(self, code: str) -> ExecuteOutput:
        """
        Execute TypeScript code that calls namespaced functions.

        This method runs TypeScript code in a secure Deno sandbox with access to
        all registered functions (both local tools and MCP server functions).

        Args:
            code: TypeScript code to execute. Must include an async `run()` function
                that serves as the entry point. Functions must be called with their
                namespace prefix (e.g., 'Weather.getCurrentWeather()').

        Returns:
            ExecuteOutput: An object containing execution results with attributes:
                - result: The value returned from the run() function
                - logs: Array of console.log() outputs
                - markdown(): Method to format output as markdown

        Raises:
            SessionError: If called before establishing a session via connect().
            TimeoutError: If execution exceeds the configured timeout (default 30s).

        Notes:
            - Code must define an `async function run()` as the entry point
            - Functions MUST be called as 'Namespace.functionName'
            - Only functions from list_functions() are available
            - No access to fetch(), fs, or other standard Node/Deno APIs
            - Variables don't persist between execute() calls
            - Return values are already parsed objects, not JSON strings

        Example:
            >>> async with Pctx() as pctx:
            ...     code = '''
            ...     async function run() {
            ...         const result = await Weather.getCurrentWeather({ city: "NYC" });
            ...         console.log("Temperature:", result.temp);
            ...         return { temperature: result.temp };
            ...     }
            ...     '''
            ...     output = await pctx.execute(code)
            ...     print(output.markdown())  # Formatted results with logs
        """
        if self._session_id is None:
            raise SessionError(
                "No code mode session exists, run Pctx(...).connect() before calling"
            )
        return await self._ws_client.execute_code(
            self._session_id, code, timeout=self._execute_timeout
        )

    # ========== Registrations ==========

    async def _register_tools(self, configs: list[ToolConfig]):
        res = await self._client.post("/register/tools", json={"tools": configs})
        res.raise_for_status()

    async def _register_servers(self, configs: list[ServerConfig]):
        res = await self._client.post("/register/servers", json={"servers": configs})
        res.raise_for_status()

    def langchain_tools(self) -> "list[LangchainBaseTool]":
        """
        Expose PCTX code mode tools as langchain tools

        Requires the 'langchain' extra to be installed:
            pip install pctx[langchain]

        Raises:
            ImportError: If langchain is not installed.
        """
        try:
            from langchain_core.tools import tool as langchain_tool
        except ImportError as e:
            raise ImportError(
                "LangChain is not installed. Install it with: pip install pctx[langchain]"
            ) from e

        @langchain_tool(description=CODE_MODE_TOOL_DESCRIPTIONS["list_functions"])
        async def list_functions() -> str:
            return (await self.list_functions()).code

        @langchain_tool(description=CODE_MODE_TOOL_DESCRIPTIONS["get_function_details"])
        async def get_function_details(functions: list[str]) -> str:
            return (
                await self.get_function_details(
                    functions,
                )
            ).code

        @langchain_tool(description=CODE_MODE_TOOL_DESCRIPTIONS["execute"])
        async def execute(code: str) -> str:
            return (await self.execute(code)).markdown()

        return [list_functions, get_function_details, execute]

    def crewai_tools(self) -> "list[CrewAiBaseTool]":
        """
        Expose PCTX code mode tools as crewai tools

        Requires the 'crewai' extra to be installed:
            pip install pctx[crewai]

        Raises:
            ImportError: If crewai is not installed.
        """
        try:
            from crewai.tools import BaseTool as CrewAiBaseTool
        except ImportError as e:
            raise ImportError(
                "CrewAI is not installed. Install it with: pip install pctx[crewai]"
            ) from e

        import asyncio

        # Capture the current event loop for later use from threads
        try:
            main_loop = asyncio.get_running_loop()
        except RuntimeError:
            main_loop = None

        class ListFunctionsTool(CrewAiBaseTool):
            name: str = "list_functions"
            description: str = CODE_MODE_TOOL_DESCRIPTIONS["list_functions"]

            def _run(_self) -> str:
                # When called from CrewAI's thread pool, use the main event loop
                if main_loop is not None:
                    future = asyncio.run_coroutine_threadsafe(
                        self.list_functions(), main_loop
                    )
                    return future.result(timeout=30).code
                else:
                    # No event loop captured, create a new one
                    return asyncio.run(self.list_functions()).code

        class GetFunctionDetailsTool(CrewAiBaseTool):
            name: str = "get_function_details"
            description: str = CODE_MODE_TOOL_DESCRIPTIONS["get_function_details"]
            args_schema: type[BaseModel] = GetFunctionDetailsInput

            def _run(_self, functions: list[str]) -> str:
                # When called from CrewAI's thread pool, use the main event loop
                if main_loop is not None:
                    future = asyncio.run_coroutine_threadsafe(
                        self.get_function_details(functions=functions), main_loop
                    )
                    return future.result(timeout=30).code
                else:
                    # No event loop captured, create a new one
                    return asyncio.run(
                        self.get_function_details(functions=functions)
                    ).code

        class ExecuteTool(CrewAiBaseTool):
            name: str = "execute"
            description: str = CODE_MODE_TOOL_DESCRIPTIONS["execute"]
            args_schema: type[BaseModel] = ExecuteInput

            def _run(_self, code: str) -> str:
                # When called from CrewAI's thread pool, use the main event loop
                if main_loop is not None:
                    future = asyncio.run_coroutine_threadsafe(
                        self.execute(code=code), main_loop
                    )
                    return future.result(timeout=self._execute_timeout).markdown()
                else:
                    # No event loop captured, create a new one
                    return asyncio.run(self.execute(code=code)).markdown()

        return [ListFunctionsTool(), GetFunctionDetailsTool(), ExecuteTool()]

    def openai_agents_tools(self) -> "list[FunctionTool]":
        """
        Expose PCTX code mode tools as OpenAI Agents SDK function tools

        Requires the 'openai' extra to be installed:
            pip install pctx[openai]

        Returns:
            List of function tools compatible with OpenAI Agents SDK

        Raises:
            ImportError: If openai is not installed.
        """
        try:
            from agents import function_tool
        except ImportError as e:
            raise ImportError(
                "OpenAI Agents SDK is not installed. Install it with: pip install pctx[openai]"
            ) from e

        # OpenAI Agents SDK uses function decorators to create tools
        # We need to create wrapper functions that call our async methods

        async def list_functions_wrapper() -> str:
            return (await self.list_functions()).code

        async def get_function_details_wrapper(functions: list[str]) -> str:
            return (await self.get_function_details(functions)).code

        async def execute_wrapper(code: str) -> str:
            return (await self.execute(code)).markdown()

        # Set docstrings and apply decorator
        list_functions_wrapper.__doc__ = CODE_MODE_TOOL_DESCRIPTIONS["list_functions"]
        get_function_details_wrapper.__doc__ = f"""{CODE_MODE_TOOL_DESCRIPTIONS["get_function_details"]}

Args:
    functions: List of function names in 'namespace.functionName' format"""
        execute_wrapper.__doc__ = f"""{CODE_MODE_TOOL_DESCRIPTIONS["execute"]}

Args:
    code: TypeScript code to execute"""

        # Apply the function_tool decorator
        list_functions_tool = function_tool(name_override="list_functions")(
            list_functions_wrapper
        )
        get_function_details_tool = function_tool(name_override="get_function_details")(
            get_function_details_wrapper
        )
        execute_tool = function_tool(name_override="execute")(execute_wrapper)

        return [list_functions_tool, get_function_details_tool, execute_tool]

    def pydantic_ai_tools(self) -> "list[PydanticAITool]":
        """
        Expose PCTX code mode tools as Pydantic AI tools

        Requires the 'pydantic-ai' extra to be installed:
            pip install pctx[pydantic-ai]

        Raises:
            ImportError: If pydantic-ai is not installed.
        """
        try:
            from pydantic_ai.tools import Tool as PydanticAITool
        except ImportError as e:
            raise ImportError(
                "Pydantic AI is not installed. Install it with: pip install pctx[pydantic-ai]"
            ) from e

        # Pydantic AI uses function decorators to create tools
        # We need to create wrapper functions that call our async methods

        async def list_functions_wrapper() -> str:
            return (await self.list_functions()).code

        async def get_function_details_wrapper(functions: list[str]) -> str:
            return (await self.get_function_details(functions)).code

        async def execute_wrapper(code: str) -> str:
            return (await self.execute(code)).markdown()

        # Create Pydantic AI tools using the Tool class with explicit descriptions
        tools = [
            PydanticAITool(
                list_functions_wrapper,
                name="list_functions",
                description=CODE_MODE_TOOL_DESCRIPTIONS["list_functions"],
            ),
            PydanticAITool(
                get_function_details_wrapper,
                name="get_function_details",
                description=CODE_MODE_TOOL_DESCRIPTIONS["get_function_details"],
            ),
            PydanticAITool(
                execute_wrapper,
                name="execute",
                description=CODE_MODE_TOOL_DESCRIPTIONS["execute"],
            ),
        ]

        return tools


CODE_MODE_TOOL_DESCRIPTIONS = {
    "list_functions": """ALWAYS USE THIS TOOL FIRST to list all available functions organized by namespace.

WORKFLOW:
1. Start here - Call this tool to see what functions are available
2. Then call get_function_details() for specific functions you need to understand
3. Finally call execute() to run your TypeScript code

This returns function signatures without full details.""",
    "get_function_details": """Get detailed information about specific functions you want to use.

WHEN TO USE: After calling list_functions(), use this to learn about parameter types, return values, and usage for specific functions.

REQUIRED FORMAT: Functions must be specified as 'namespace.functionName' (e.g., 'Namespace.apiPostSearch')

This tool is lightweight and only returns details for the functions you request, avoiding unnecessary token usage.
Only request details for functions you actually plan to use in your code.

NOTE ON RETURN TYPES:
- If a function returns Promise<any>, the MCP server didn't provide an output schema
- The actual value is a parsed object (not a string) - access properties directly
- Don't use JSON.parse() on the results - they're already JavaScript objects""",
    "execute": """Execute TypeScript code that calls namespaced functions. USE THIS LAST after list_functions() and get_function_details().

TOKEN USAGE WARNING: This tool could return LARGE responses if your code returns big objects.
To minimize tokens:
- Filter/map/reduce data IN YOUR CODE before returning
- Only return specific fields you need (e.g., return {id: result.id, count: items.length})
- Use console.log() for intermediate results instead of returning everything
- Avoid returning full API responses - extract just what you need

REQUIRED CODE STRUCTURE:
async function run() {
    // Your code here
    // Call namespace.functionName() - MUST include namespace prefix
    // Process data here to minimize return size
    return onlyWhatYouNeed; // Keep this small!
}

IMPORTANT RULES:
- Functions MUST be called as 'Namespace.functionName' (e.g., 'Notion.apiPostSearch')
- Only functions from list_functions() are available - no fetch(), fs, or other Node/Deno APIs
- Variables don't persist between execute() calls - return or log anything you need later
- Add console.log() statements between API calls to track progress if errors occur
- Code runs in an isolated Deno sandbox with restricted network access

RETURN TYPE NOTE:
- Functions without output schemas show Promise<any> as return type
- The actual runtime value is already a parsed JavaScript object, NOT a JSON string
- Do NOT call JSON.parse() on results - they're already objects
- Access properties directly (e.g., result.data) or inspect with console.log() first
- If you see 'Promise<any>', the structure is unknown - log it to see what's returned""",
}
