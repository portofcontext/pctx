"""
PCTX Client

Main client for executing code with both MCP tools and local Python tools.
"""

from typing import TYPE_CHECKING
from urllib.parse import urlparse

from httpx import AsyncClient
from pydantic import BaseModel

from pctx_client._tool import AsyncTool, Tool
from pctx_client._utils import to_snake_case
from pctx_client._websocket_client import WebSocketClient
from pctx_client.exceptions import ConnectionError, SessionError
from pctx_client.models import (
    ExecuteBashInput,
    ExecuteInput,
    ExecuteOutput,
    GetFunctionDetailsInput,
    GetFunctionDetailsOutput,
    ListedFunction,
    ListFunctionsOutput,
    ServerConfig,
    ToolConfig,
)
from pctx_client.tool_descriptions import PRESCRIPTIVE_DESCRIPTIONS
from pctx_client.tools import ModeString, ToolName, get_toolset_from_mode

if TYPE_CHECKING:
    try:
        from agents import FunctionTool
        from bm25s import BM25
        from crewai.tools import BaseTool as CrewAiBaseTool
        from langchain_core.tools import BaseTool as LangchainBaseTool
        from pydantic_ai.tools import Tool as PydanticAITool
        from Stemmer import Stemmer
    except ImportError:
        pass

try:
    from bm25s import BM25, tokenize
    from Stemmer import Stemmer

    HAS_SEARCH = True
except ImportError:
    HAS_SEARCH = False


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
        api_key: str | None = None,
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

        self._ws_client = WebSocketClient(
            url=f"{ws_scheme}://{host}{parsed.path}/ws", api_key=api_key, tools=tools
        )
        self._client = AsyncClient(
            base_url=f"{http_scheme}://{host}{parsed.path}",
            headers={"x-pctx-api-key": api_key or ""},
        )
        self._session_id: str | None = None
        self._api_key = api_key

        self._tools = tools or []
        self._servers = servers or []
        self._execute_timeout = execute_timeout
        self._search_retriever = None

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
        self._client.headers.update({"x-code-mode-session": self._session_id or ""})

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

        if len(configs) > 0:
            await self._register_tools(configs)
        if len(self._servers) > 0:
            await self._register_servers(self._servers)

        # reset search to re-index
        self._search_retriever = None

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

    async def search_functions(self, query: str, k: int = 10) -> list[ListedFunction]:
        """
        Search available functions matching query.

        This is typically the first method you should call to discover what functions
        are available in the current session, including both registered local tools
        and MCP server functions.

        Args:
            query: Search query string to find relevant functions.
            k: Max number of top results to return (default: 5).

        Returns:
            list[ListedFunction]: An list of matching function signatures matching the query

        Raises:
            ImportError: If bm25s is not installed.
            SessionError: If called before establishing a session via connect().
        """

        if not HAS_SEARCH:
            raise ImportError(
                "bm25s is not installed. Install it with: pip install pctx[bm25s]"
            )

        if self._session_id is None:
            raise SessionError(
                "No code mode session exists, run Pctx(...).connect() before calling"
            )

        stemmer = Stemmer("english")

        if self._search_retriever is None:
            self._functions = (await self.list_functions()).functions
            corpus = [
                f"{to_snake_case(function.namespace).replace('_', ' ')}.{to_snake_case(function.name).replace('_', ' ')}: {function.description}"
                for function in self._functions
            ]

            corpus_tokens = tokenize(corpus, stopwords="en", stemmer=stemmer)
            self._search_retriever = BM25()
            self._search_retriever.index(corpus_tokens)

        query_tokens = tokenize([query], stopwords="en", stemmer=stemmer)
        actual_k = min(k, len(self._functions))
        results, scores = self._search_retriever.retrieve(query_tokens, k=actual_k)
        tools = []
        for i in range(results.shape[1]):
            tool = self._functions[results[0, i]]
            score = scores[0, i]
            if score > 0:
                tools.append(tool)
        return tools

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

    async def execute_bash(self, command: str) -> ExecuteOutput:
        """
        Execute a bash command in the virtual filesystem.

        The bash environment has access to the virtual filesystem populated with
        tool definitions, README.md, and TypeScript definition files.

        Args:
            command: Bash command to execute (e.g., "ls /", "cat /README.md", "grep -r 'function' /")

        Returns:
            ExecuteOutput with success status, stdout, stderr, and optional output

        Raises:
            SessionError: If not connected to a session

        Example:
            >>> async with Pctx() as pctx:
            ...     # List files in the virtual filesystem
            ...     output = await pctx.execute_bash("ls /")
            ...     print(output.stdout)  # Shows README.md, index.d.ts, etc.
            ...
            ...     # Read the README to see available functions
            ...     output = await pctx.execute_bash("cat /README.md")
            ...     print(output.stdout)  # Shows available functions
        """
        if self._session_id is None:
            raise SessionError(
                "No code mode session exists, run Pctx(...).connect() before calling"
            )
        response = await self._client.post(
            "/code-mode/execute-bash", json={"command": command}
        )
        response.raise_for_status()
        return ExecuteOutput.model_validate(response.json())

    # ========== Registrations ==========

    async def _register_tools(self, configs: list[ToolConfig]):
        res = await self._client.post("/register/tools", json={"tools": configs})
        res.raise_for_status()

    async def _register_servers(self, configs: list[ServerConfig]):
        res = await self._client.post("/register/servers", json={"servers": configs})
        res.raise_for_status()

    def _search_functions_result_to_string(
        self, functions: list[ListedFunction]
    ) -> str:
        return "\n".join(
            [
                f"{func.namespace}.{func.name}: {func.description or ''}"
                for func in functions
            ]
        )

    def langchain_tools(
        self,
        mode: ModeString | ToolConfig = "list_get_execute",
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[LangchainBaseTool]":
        """
        Expose PCTX tools as LangChain tools

        Args:
            mode: Tool mode configuration. Either:
                  - "list_get_execute" (default): list_functions, search_functions,
                    get_function_details, execute
                  - "fs": execute_bash, execute_typescript
                  - ToolConfig: Custom tool selection
            descriptions: Optional custom descriptions to override defaults.
                          Only used when mode is a string.

        Requires the 'langchain' extra to be installed:
            pip install pctx[langchain]

        Raises:
            ImportError: If langchain is not installed.

        Examples:
            Pre-bundled modes:
            >>> tools = pctx.langchain_tools()  # default: list_get_execute
            >>> tools = pctx.langchain_tools("fs")

            Override descriptions:
            >>> tools = pctx.langchain_tools("list_get_execute", descriptions={"execute": "Custom"})

            Full control:
            >>> from pctx_client.tools import ToolConfig
            >>> tools = pctx.langchain_tools(ToolConfig(tools=["execute_bash", "list_functions"]))
        """
        try:
            from langchain_core.tools import tool as langchain_tool
        except ImportError as e:
            raise ImportError(
                "LangChain is not installed. Install it with: pip install pctx[langchain]"
            ) from e

        # Convert mode string to ToolConfig if needed
        if isinstance(mode, str):
            toolset = get_toolset_from_mode(mode, descriptions)
        else:
            toolset = mode

        # Helper to get description with fallback
        def get_desc(key: str) -> str:
            if toolset.descriptions:
                return toolset.descriptions.get(key, CODE_MODE_TOOL_DESCRIPTIONS[key])
            return CODE_MODE_TOOL_DESCRIPTIONS[key]

        tools = []

        # Build tools based on toolset configuration using registry
        from pctx_client._tool_registry import TOOL_REGISTRY

        for tool_name in toolset.tools:
            # Validate tool exists in registry
            if tool_name not in TOOL_REGISTRY:
                raise ValueError(
                    f"Unknown tool: {tool_name}. Valid tools: {sorted(TOOL_REGISTRY)}"
                )

            # Skip search_functions if BM25 not installed
            if tool_name == "search_functions" and not HAS_SEARCH:
                continue

            # Create framework-specific tool
            tool = self._create_langchain_tool(
                tool_name, get_desc(tool_name), langchain_tool
            )
            tools.append(tool)

        return tools

    def _create_langchain_tool(
        self, tool_name: ToolName, description: str, langchain_tool
    ):
        """Factory method to create a LangChain tool for the given tool name"""
        if tool_name == "execute_bash":

            @langchain_tool(description=description)
            async def execute_bash(command: str) -> str:
                return (await self.execute_bash(command)).markdown()

            return execute_bash

        elif tool_name == "execute_typescript":

            @langchain_tool(description=description)
            async def execute_typescript(code: str) -> str:
                return (await self.execute(code)).markdown()

            return execute_typescript

        elif tool_name == "list_functions":

            @langchain_tool(description=description)
            async def list_functions() -> str:
                return (await self.list_functions()).code

            return list_functions

        elif tool_name == "search_functions":

            @langchain_tool(description=description)
            async def search_functions(query: str, k: int = 10) -> str:
                functions = await self.search_functions(query, k)
                return self._search_functions_result_to_string(functions)

            return search_functions

        elif tool_name == "get_function_details":

            @langchain_tool(description=description)
            async def get_function_details(functions: list[str]) -> str:
                return (
                    await self.get_function_details(
                        functions,
                    )
                ).code

            return get_function_details

        elif tool_name == "execute":

            @langchain_tool(description=description)
            async def execute(code: str) -> str:
                return (await self.execute(code)).markdown()

            return execute

        else:
            raise ValueError(f"Unsupported LangChain tool: {tool_name}")

    def crewai_tools(
        self,
        mode: ModeString | ToolConfig = "list_get_execute",
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[CrewAiBaseTool]":
        """
        Expose PCTX tools as CrewAI tools

        Args:
            mode: Tool mode configuration. Either:
                  - "list_get_execute" (default): list_functions, search_functions,
                    get_function_details, execute
                  - "fs": execute_bash, execute_typescript
                  - ToolConfig: Custom tool selection
            descriptions: Optional custom descriptions to override defaults.
                          Only used when mode is a string.

        Requires the 'crewai' extra to be installed:
            pip install pctx[crewai]

        Raises:
            ImportError: If crewai is not installed.

        Examples:
            Pre-bundled modes:
            >>> tools = pctx.crewai_tools()  # default: list_get_execute
            >>> tools = pctx.crewai_tools("fs")

            Override descriptions:
            >>> tools = pctx.crewai_tools("list_get_execute", descriptions={"execute": "Custom"})

            Full control:
            >>> from pctx_client.tools import ToolConfig
            >>> tools = pctx.crewai_tools(ToolConfig(tools=["execute_bash", "list_functions"]))
        """
        try:
            from crewai.tools import BaseTool as CrewAiBaseTool
        except ImportError as e:
            raise ImportError(
                "CrewAI is not installed. Install it with: pip install pctx[crewai]"
            ) from e

        # Convert mode string to ToolConfig if needed
        if isinstance(mode, str):
            toolset = get_toolset_from_mode(mode, descriptions)
        else:
            toolset = mode

        # Helper to get description with fallback
        def get_desc(key: str) -> str:
            if toolset.descriptions:
                return toolset.descriptions.get(key, CODE_MODE_TOOL_DESCRIPTIONS[key])
            return CODE_MODE_TOOL_DESCRIPTIONS[key]

        tools = []
        import asyncio

        # Capture the current event loop for later use from threads
        try:
            main_loop = asyncio.get_running_loop()
        except RuntimeError:
            main_loop = None

        # Build tools based on toolset configuration using registry
        from pctx_client._tool_registry import TOOL_REGISTRY

        for tool_name in toolset.tools:
            # Validate tool exists in registry
            if tool_name not in TOOL_REGISTRY:
                raise ValueError(
                    f"Unknown tool: {tool_name}. Valid tools: {sorted(TOOL_REGISTRY)}"
                )

            # Skip search_functions if BM25 not installed
            if tool_name == "search_functions" and not HAS_SEARCH:
                continue

            # Create framework-specific tool
            tool = self._create_crewai_tool(
                tool_name, get_desc(tool_name), CrewAiBaseTool, main_loop
            )
            tools.append(tool)

        return tools

    def _create_crewai_tool(
        self, tool_name: ToolName, description: str, CrewAiBaseTool, main_loop
    ):
        """Factory method to create a CrewAI tool for the given tool name"""
        import asyncio

        # Capture description in local scope for class attribute access
        desc = description

        if tool_name == "execute_bash":

            class ExecuteBashTool(CrewAiBaseTool):
                name: str = "execute_bash"
                description: str = desc
                args_schema: type[BaseModel] = ExecuteBashInput

                def _run(_self, command: str) -> str:
                    if main_loop is not None:
                        future = asyncio.run_coroutine_threadsafe(
                            self.execute_bash(command), main_loop
                        )
                        return future.result(timeout=30).markdown()
                    else:
                        return asyncio.run(self.execute_bash(command)).markdown()

            return ExecuteBashTool()

        elif tool_name == "execute_typescript":

            class ExecuteTypeScriptTool(CrewAiBaseTool):
                name: str = "execute_typescript"
                description: str = desc
                args_schema: type[BaseModel] = ExecuteInput

                def _run(_self, code: str) -> str:
                    if main_loop is not None:
                        future = asyncio.run_coroutine_threadsafe(
                            self.execute(code), main_loop
                        )
                        return future.result(timeout=self._execute_timeout).markdown()
                    else:
                        return asyncio.run(self.execute(code)).markdown()

            return ExecuteTypeScriptTool()

        elif tool_name == "list_functions":

            class ListFunctionsTool(CrewAiBaseTool):
                name: str = "list_functions"
                description: str = desc

                def _run(_self) -> str:
                    if main_loop is not None:
                        future = asyncio.run_coroutine_threadsafe(
                            self.list_functions(), main_loop
                        )
                        return future.result(timeout=30).code
                    else:
                        return asyncio.run(self.list_functions()).code

            return ListFunctionsTool()

        elif tool_name == "search_functions":

            class SearchFunctionsTool(CrewAiBaseTool):
                name: str = "search_functions"
                description: str = desc

                def _run(_self, query: str, k: int = 10) -> str:
                    if main_loop is not None:
                        future = asyncio.run_coroutine_threadsafe(
                            self.search_functions(query, k), main_loop
                        )
                        return self._search_functions_result_to_string(
                            future.result(timeout=30)
                        )
                    else:
                        return self._search_functions_result_to_string(
                            asyncio.run(self.search_functions(query, k))
                        )

            return SearchFunctionsTool()

        elif tool_name == "get_function_details":

            class GetFunctionDetailsTool(CrewAiBaseTool):
                name: str = "get_function_details"
                description: str = desc
                args_schema: type[BaseModel] = GetFunctionDetailsInput

                def _run(_self, functions: list[str]) -> str:
                    if main_loop is not None:
                        future = asyncio.run_coroutine_threadsafe(
                            self.get_function_details(functions=functions), main_loop
                        )
                        return future.result(timeout=30).code
                    else:
                        return asyncio.run(
                            self.get_function_details(functions=functions)
                        ).code

            return GetFunctionDetailsTool()

        elif tool_name == "execute":

            class ExecuteTool(CrewAiBaseTool):
                name: str = "execute"
                description: str = desc
                args_schema: type[BaseModel] = ExecuteInput

                def _run(_self, code: str) -> str:
                    if main_loop is not None:
                        future = asyncio.run_coroutine_threadsafe(
                            self.execute(code=code), main_loop
                        )
                        return future.result(timeout=self._execute_timeout).markdown()
                    else:
                        return asyncio.run(self.execute(code=code)).markdown()

            return ExecuteTool()

        else:
            raise ValueError(f"Unsupported CrewAI tool: {tool_name}")

    def openai_agents_tools(
        self,
        mode: ModeString | ToolConfig = "list_get_execute",
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[FunctionTool]":
        """
        Expose PCTX tools as OpenAI Agents SDK function tools

        Args:
            mode: Tool mode configuration. Either:
                  - "list_get_execute" (default): list_functions, search_functions,
                    get_function_details, execute
                  - "fs": execute_bash, execute_typescript
                  - ToolConfig: Custom tool selection
            descriptions: Optional custom descriptions to override defaults.
                          Only used when mode is a string.

        Requires the 'openai' extra to be installed:
            pip install pctx[openai]

        Returns:
            List of function tools compatible with OpenAI Agents SDK

        Raises:
            ImportError: If openai is not installed.

        Examples:
            Pre-bundled modes:
            >>> tools = pctx.openai_agents_tools()  # default: list_get_execute
            >>> tools = pctx.openai_agents_tools("fs")

            Override descriptions:
            >>> tools = pctx.openai_agents_tools("list_get_execute", descriptions={"execute": "Custom"})

            Full control:
            >>> from pctx_client.tools import ToolConfig
            >>> tools = pctx.openai_agents_tools(ToolConfig(tools=["execute_bash", "list_functions"]))
        """
        try:
            from agents import function_tool
        except ImportError as e:
            raise ImportError(
                "OpenAI Agents SDK is not installed. Install it with: pip install pctx[openai]"
            ) from e

        # Convert mode string to ToolConfig if needed
        if isinstance(mode, str):
            toolset = get_toolset_from_mode(mode, descriptions)
        else:
            toolset = mode

        # Helper to get description with fallback
        def get_desc(key: str) -> str:
            if toolset.descriptions:
                return toolset.descriptions.get(key, CODE_MODE_TOOL_DESCRIPTIONS[key])
            return CODE_MODE_TOOL_DESCRIPTIONS[key]

        tools = []

        # Build tools based on toolset configuration using registry
        from pctx_client._tool_registry import TOOL_REGISTRY

        for tool_name in toolset.tools:
            # Validate tool exists in registry
            if tool_name not in TOOL_REGISTRY:
                raise ValueError(
                    f"Unknown tool: {tool_name}. Valid tools: {sorted(TOOL_REGISTRY)}"
                )

            # Skip search_functions if BM25 not installed
            if tool_name == "search_functions" and not HAS_SEARCH:
                continue

            # Create framework-specific tool
            tool = self._create_openai_agents_tool(
                tool_name, get_desc(tool_name), function_tool
            )
            tools.append(tool)

        return tools

    def _create_openai_agents_tool(
        self, tool_name: ToolName, description: str, function_tool
    ):
        """Factory method to create an OpenAI Agents SDK tool for the given tool name"""
        if tool_name == "execute_bash":

            async def execute_bash_wrapper(command: str) -> str:
                return (await self.execute_bash(command)).markdown()

            execute_bash_wrapper.__doc__ = f"""{description}

Args:
    command: Bash command to execute"""

            return function_tool(name_override="execute_bash")(execute_bash_wrapper)

        elif tool_name == "execute_typescript":

            async def execute_typescript_wrapper(code: str) -> str:
                return (await self.execute(code)).markdown()

            execute_typescript_wrapper.__doc__ = f"""{description}

Args:
    code: TypeScript code to execute"""

            return function_tool(name_override="execute_typescript")(
                execute_typescript_wrapper
            )

        elif tool_name == "list_functions":

            async def list_functions_wrapper() -> str:
                return (await self.list_functions()).code

            list_functions_wrapper.__doc__ = description
            return function_tool(name_override="list_functions")(list_functions_wrapper)

        elif tool_name == "search_functions":

            async def search_functions_wrapper(query: str, k: int = 10) -> str:
                functions = await self.search_functions(query, k)
                return self._search_functions_result_to_string(functions)

            search_functions_wrapper.__doc__ = description
            return function_tool(name_override="search_functions")(
                search_functions_wrapper
            )

        elif tool_name == "get_function_details":

            async def get_function_details_wrapper(functions: list[str]) -> str:
                return (await self.get_function_details(functions)).code

            get_function_details_wrapper.__doc__ = f"""{description}

Args:
    functions: List of function names in 'namespace.functionName' format"""

            return function_tool(name_override="get_function_details")(
                get_function_details_wrapper
            )

        elif tool_name == "execute":

            async def execute_wrapper(code: str) -> str:
                return (await self.execute(code)).markdown()

            execute_wrapper.__doc__ = f"""{description}

Args:
    code: TypeScript code to execute"""

            return function_tool(name_override="execute")(execute_wrapper)

        else:
            raise ValueError(f"Unsupported OpenAI Agents tool: {tool_name}")

    def pydantic_ai_tools(
        self,
        mode: ModeString | ToolConfig = "list_get_execute",
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[PydanticAITool]":
        """
        Expose PCTX tools as Pydantic AI tools

        Args:
            mode: Tool mode configuration. Either:
                  - "list_get_execute" (default): list_functions, search_functions,
                    get_function_details, execute
                  - "fs": execute_bash, execute_typescript
                  - ToolConfig: Custom tool selection
            descriptions: Optional custom descriptions to override defaults.
                          Only used when mode is a string.

        Requires the 'pydantic-ai' extra to be installed:
            pip install pctx[pydantic-ai]

        Raises:
            ImportError: If pydantic-ai is not installed.

        Examples:
            Pre-bundled modes:
            >>> tools = pctx.pydantic_ai_tools()  # default: list_get_execute
            >>> tools = pctx.pydantic_ai_tools("fs")

            Override descriptions:
            >>> tools = pctx.pydantic_ai_tools("list_get_execute", descriptions={"execute": "Custom"})

            Full control:
            >>> from pctx_client.tools import ToolConfig
            >>> tools = pctx.pydantic_ai_tools(ToolConfig(tools=["execute_bash", "list_functions"]))
        """
        try:
            from pydantic_ai.tools import Tool as PydanticAITool
        except ImportError as e:
            raise ImportError(
                "Pydantic AI is not installed. Install it with: pip install pctx[pydantic-ai]"
            ) from e

        # Convert mode string to ToolConfig if needed
        if isinstance(mode, str):
            toolset = get_toolset_from_mode(mode, descriptions)
        else:
            toolset = mode

        # Helper to get description with fallback
        def get_desc(key: str) -> str:
            if toolset.descriptions:
                return toolset.descriptions.get(key, CODE_MODE_TOOL_DESCRIPTIONS[key])
            return CODE_MODE_TOOL_DESCRIPTIONS[key]

        tools = []

        # Build tools based on toolset configuration using registry
        from pctx_client._tool_registry import TOOL_REGISTRY

        for tool_name in toolset.tools:
            # Validate tool exists in registry
            if tool_name not in TOOL_REGISTRY:
                raise ValueError(
                    f"Unknown tool: {tool_name}. Valid tools: {sorted(TOOL_REGISTRY)}"
                )

            # Skip search_functions if BM25 not installed
            if tool_name == "search_functions" and not HAS_SEARCH:
                continue

            # Create framework-specific tool
            tool = self._create_pydantic_ai_tool(
                tool_name, get_desc(tool_name), PydanticAITool
            )
            tools.append(tool)

        return tools

    def _create_pydantic_ai_tool(
        self, tool_name: ToolName, description: str, PydanticAITool
    ):
        """Factory method to create a Pydantic AI tool for the given tool name"""
        if tool_name == "execute_bash":

            async def execute_bash_wrapper(command: str) -> str:
                return (await self.execute_bash(command)).markdown()

            return PydanticAITool(
                execute_bash_wrapper,
                name="execute_bash",
                description=description,
            )

        elif tool_name == "execute_typescript":

            async def execute_typescript_wrapper(code: str) -> str:
                return (await self.execute(code)).markdown()

            return PydanticAITool(
                execute_typescript_wrapper,
                name="execute_typescript",
                description=description,
            )

        elif tool_name == "list_functions":

            async def list_functions_wrapper() -> str:
                return (await self.list_functions()).code

            return PydanticAITool(
                list_functions_wrapper,
                name="list_functions",
                description=description,
            )

        elif tool_name == "search_functions":

            async def search_functions_wrapper(query: str, k: int = 10) -> str:
                functions = await self.search_functions(query, k)
                return self._search_functions_result_to_string(functions)

            return PydanticAITool(
                search_functions_wrapper,
                name="search_functions",
                description=description,
            )

        elif tool_name == "get_function_details":

            async def get_function_details_wrapper(functions: list[str]) -> str:
                return (await self.get_function_details(functions)).code

            return PydanticAITool(
                get_function_details_wrapper,
                name="get_function_details",
                description=description,
            )

        elif tool_name == "execute":

            async def execute_wrapper(code: str) -> str:
                return (await self.execute(code)).markdown()

            return PydanticAITool(
                execute_wrapper,
                name="execute",
                description=description,
            )

        else:
            raise ValueError(f"Unsupported Pydantic AI tool: {tool_name}")


# Import tool descriptions - change this to experiment with different styles
# Options: PRESCRIPTIVE_DESCRIPTIONS, TERMINAL_STYLE_DESCRIPTIONS
# See pctx_client/tool_descriptions/README.md for details
CODE_MODE_TOOL_DESCRIPTIONS = PRESCRIPTIVE_DESCRIPTIONS
