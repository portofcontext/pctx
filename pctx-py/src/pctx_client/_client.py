"""
PCTX Client

Main client for executing code with both MCP tools and local Python tools.
"""

import asyncio
import warnings
from typing import TYPE_CHECKING, Any
from urllib.parse import urlparse

from httpx import AsyncClient
from pydantic import BaseModel

from pctx_client._tool import AsyncTool, Tool
from pctx_client._utils import HAS_SEARCH, to_snake_case
from pctx_client._websocket_client import WebSocketClient
from pctx_client.descriptions import get_tool_description
from pctx_client.exceptions import ConnectionError, SessionError
from pctx_client.models import (
    ExecuteBashInput,
    ExecuteBashOutput,
    ExecuteTypescriptInput,
    ExecuteTypescriptOutput,
    GetFunctionDetailsInput,
    GetFunctionDetailsOutput,
    ListedFunction,
    ListFunctionsOutput,
    ServerConfig,
    ToolConfig,
    ToolDisclosure,
    ToolDisclosureName,
    ToolName,
)

if TYPE_CHECKING:
    try:
        from agents import FunctionTool
        from bm25s import BM25
        from claude_agent_sdk import SdkMcpTool as ClaudeSdkMcpTool
        from crewai.tools import BaseTool as CrewAiBaseTool
        from langchain_core.tools import BaseTool as LangchainBaseTool
        from pydantic_ai.tools import Tool as PydanticAITool
        from Stemmer import Stemmer
    except ImportError:
        pass

try:
    from bm25s import BM25, tokenize
    from Stemmer import Stemmer
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

    async def execute_typescript(
        self,
        code: str,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
    ) -> ExecuteTypescriptOutput:
        """
        Execute TypeScript code that calls namespaced functions.

        This method runs TypeScript code in a secure Deno sandbox with access to
        all registered functions (both local tools and MCP server functions).

        Args:
            code: TypeScript code to execute. Must include an async `run()` function
                that serves as the entry point. Functions must be called with their
                namespace prefix (e.g., 'Weather.getCurrentWeather()').

        Returns:
            ExecuteTypescriptOutput: An object containing execution results with attributes:
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
            ...     output = await pctx.execute_typescript(code)
            ...     print(output.markdown())  # Formatted results with logs
        """

        if self._session_id is None:
            raise SessionError(
                "No code mode session exists, run Pctx(...).connect() before calling"
            )
        return await self._ws_client.execute_typescript(
            self._session_id,
            code,
            disclosure=ToolDisclosure(disclosure),
            timeout=self._execute_timeout,
        )

    async def execute(
        self,
        code: str,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
    ) -> ExecuteTypescriptOutput:
        """Deprecated alias for execute_typescript."""
        warnings.warn(
            "execute() is deprecated, use execute_typescript() instead",
            DeprecationWarning,
            stacklevel=2,
        )
        return await self.execute_typescript(code, disclosure=disclosure)

    async def execute_bash(self, command: str) -> ExecuteBashOutput:
        """
        Execute a bash command in the virtual filesystem.

        The bash environment has access to the virtual filesystem populated with
        tool definitions, README.md, and TypeScript definition files.

        Args:
            command: Bash command to execute (e.g., "ls /", "cat /README.md", "grep -r 'function' /")

        Returns:
            ExecuteBashOutput with exit code, stdout, and stderr

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
        return ExecuteBashOutput.model_validate(response.json())

    # ========== Registrations ==========

    async def _register_tools(self, configs: list[ToolConfig]):
        res = await self._client.post("/register/tools", json={"tools": configs})
        res.raise_for_status()

    async def _register_servers(self, configs: list[ServerConfig]):
        res = await self._client.post("/register/servers", json={"servers": configs})
        res.raise_for_status()

    # ========== Utils ==========

    def _search_functions_result_to_string(
        self, functions: list[ListedFunction]
    ) -> str:
        return "\n".join(
            [
                f"{func.namespace}.{func.name}: {func.description or ''}"
                for func in functions
            ]
        )

    # ========== Frameworks ==========

    def langchain_tools(
        self,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[LangchainBaseTool]":
        """
        Expose PCTX tools as LangChain tools

        Args:
            disclosure: Controls which tools are exposed and how function context is
                provided to the model. CATALOG (default) exposes list_functions,
                get_function_details, and execute_typescript — the agent discovers
                and retrieves function signatures before executing. FS exposes
                execute_bash and execute_typescript — the agent browses the virtual
                filesystem directly.
            descriptions: Optional custom descriptions to override defaults.

        Requires the 'langchain' extra to be installed:
            pip install pctx[langchain]

        Raises:
            ImportError: If langchain is not installed.

        Examples:
            >>> tools = pctx.langchain_tools()  # default: catalog
            >>> tools = pctx.langchain_tools(disclosure="filesystem")
            >>> tools = pctx.langchain_tools(descriptions={"execute_typescript": "Custom"})
        """
        disclosure = ToolDisclosure(disclosure)
        try:
            from langchain_core.tools import tool as langchain_tool
        except ImportError as e:
            raise ImportError(
                "LangChain is not installed. Install it with: pip install pctx[langchain]"
            ) from e

        # build all tools

        @langchain_tool(
            description=get_tool_description("execute_bash", overrides=descriptions)
        )
        async def execute_bash(command: str) -> str:
            return (await self.execute_bash(command)).markdown()

        @langchain_tool(
            description=get_tool_description(
                "execute_typescript", disclosure=disclosure, overrides=descriptions
            )
        )
        async def execute_typescript(code: str) -> str:
            return (
                await self.execute_typescript(code, disclosure=disclosure)
            ).markdown()

        @langchain_tool(
            description=get_tool_description("list_functions", overrides=descriptions)
        )
        async def list_functions() -> str:
            return (await self.list_functions()).code

        @langchain_tool(
            description=get_tool_description("search_functions", overrides=descriptions)
        )
        async def search_functions(query: str, k: int = 10) -> str:
            functions = await self.search_functions(query, k)
            return self._search_functions_result_to_string(functions)

        @langchain_tool(
            description=get_tool_description(
                "get_function_details", overrides=descriptions
            )
        )
        async def get_function_details(functions: list[str]) -> str:
            return (
                await self.get_function_details(
                    functions,
                )
            ).code

        all_tools = [
            execute_bash,
            execute_typescript,
            list_functions,
            search_functions,
            get_function_details,
        ]

        # filter according to disclosure
        return [t for t in all_tools if disclosure.contains_tool(t.name)]

    def crewai_tools(
        self,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[CrewAiBaseTool]":
        """
        Expose PCTX tools as CrewAI tools

        Args:
            disclosure: Controls which tools are exposed and how function context is
                provided to the model. CATALOG (default) exposes list_functions,
                get_function_details, and execute_typescript — the agent discovers
                and retrieves function signatures before executing. FS exposes
                execute_bash and execute_typescript — the agent browses the virtual
                filesystem directly.
            descriptions: Optional custom descriptions to override defaults.

        Requires the 'crewai' extra to be installed:
            pip install pctx[crewai]

        Raises:
            ImportError: If crewai is not installed.

        Examples:
            >>> tools = pctx.crewai_tools()  # default: catalog
            >>> tools = pctx.crewai_tools(disclosure="filesystem")
            >>> tools = pctx.crewai_tools(descriptions={"execute_typescript": "Custom"})
        """
        disclosure = ToolDisclosure(disclosure)
        try:
            from crewai.tools import BaseTool as CrewAiBaseTool
        except ImportError as e:
            raise ImportError(
                "CrewAI is not installed. Install it with: pip install pctx[crewai]"
            ) from e

        # Capture the current event loop for later use from threads
        try:
            main_loop = asyncio.get_running_loop()
        except RuntimeError:
            main_loop = None

        def run_async(coro, timeout: float = 30.0):
            if main_loop is not None:
                return asyncio.run_coroutine_threadsafe(coro, main_loop).result(
                    timeout=timeout
                )
            else:
                return asyncio.run(coro)

        # build all tools

        class ExecuteBashTool(CrewAiBaseTool):
            name: str = "execute_bash"
            description: str = get_tool_description(
                "execute_bash", overrides=descriptions
            )
            args_schema: type[BaseModel] = ExecuteBashInput

            def _run(_self, command: str) -> str:
                return run_async(self.execute_bash(command)).markdown()

        class ExecuteTypeScriptTool(CrewAiBaseTool):
            name: str = "execute_typescript"
            description: str = get_tool_description(
                "execute_typescript", disclosure=disclosure, overrides=descriptions
            )
            args_schema: type[BaseModel] = ExecuteTypescriptInput

            def _run(_self, code: str) -> str:
                return run_async(
                    self.execute_typescript(code, disclosure=disclosure),
                    timeout=self._execute_timeout,
                ).markdown()

        class ListFunctionsTool(CrewAiBaseTool):
            name: str = "list_functions"
            description: str = get_tool_description(
                "list_functions", overrides=descriptions
            )

            def _run(_self) -> str:
                return run_async(self.list_functions()).code

        class SearchFunctionsTool(CrewAiBaseTool):
            name: str = "search_functions"
            description: str = get_tool_description(
                "search_functions", overrides=descriptions
            )

            def _run(_self, query: str, k: int = 10) -> str:
                return self._search_functions_result_to_string(
                    run_async(self.search_functions(query, k))
                )

        class GetFunctionDetailsTool(CrewAiBaseTool):
            name: str = "get_function_details"
            description: str = get_tool_description(
                "get_function_details", overrides=descriptions
            )
            args_schema: type[BaseModel] = GetFunctionDetailsInput

            def _run(_self, functions: list[str]) -> str:
                return run_async(self.get_function_details(functions=functions)).code

        all_tools = [
            ExecuteBashTool(),
            ExecuteTypeScriptTool(),
            ListFunctionsTool(),
            SearchFunctionsTool(),
            GetFunctionDetailsTool(),
        ]

        # filter according to disclosure
        return [t for t in all_tools if disclosure.contains_tool(t.name)]

    def openai_agents_tools(
        self,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[FunctionTool]":
        """
        Expose PCTX tools as OpenAI Agents SDK function tools

        Args:
            disclosure: Controls which tools are exposed and how function context is
                provided to the model. CATALOG (default) exposes list_functions,
                get_function_details, and execute_typescript — the agent discovers
                and retrieves function signatures before executing. FS exposes
                execute_bash and execute_typescript — the agent browses the virtual
                filesystem directly.
            descriptions: Optional custom descriptions to override defaults.

        Requires the 'openai' extra to be installed:
            pip install pctx[openai]

        Raises:
            ImportError: If openai is not installed.

        Examples:
            >>> tools = pctx.openai_agents_tools()  # default: catalog
            >>> tools = pctx.openai_agents_tools(disclosure="filesystem")
            >>> tools = pctx.openai_agents_tools(descriptions={"execute_typescript": "Custom"})
        """
        disclosure = ToolDisclosure(disclosure)
        try:
            from agents import function_tool
        except ImportError as e:
            raise ImportError(
                "OpenAI Agents SDK is not installed. Install it with: pip install pctx[openai]"
            ) from e

        # build all tools

        async def execute_bash(command: str) -> str:
            return (await self.execute_bash(command)).markdown()

        execute_bash.__doc__ = get_tool_description(
            "execute_bash", overrides=descriptions
        )

        async def execute_typescript(code: str) -> str:
            return (
                await self.execute_typescript(code, disclosure=disclosure)
            ).markdown()

        execute_typescript.__doc__ = get_tool_description(
            "execute_typescript", disclosure=disclosure, overrides=descriptions
        )

        async def list_functions() -> str:
            return (await self.list_functions()).code

        list_functions.__doc__ = get_tool_description(
            "list_functions", overrides=descriptions
        )

        async def search_functions(query: str, k: int = 10) -> str:
            return self._search_functions_result_to_string(
                await self.search_functions(query, k)
            )

        search_functions.__doc__ = get_tool_description(
            "search_functions", overrides=descriptions
        )

        async def get_function_details(functions: list[str]) -> str:
            return (await self.get_function_details(functions)).code

        get_function_details.__doc__ = get_tool_description(
            "get_function_details", overrides=descriptions
        )

        all_tools = [
            function_tool(name_override="execute_bash")(execute_bash),
            function_tool(name_override="execute_typescript")(execute_typescript),
            function_tool(name_override="list_functions")(list_functions),
            function_tool(name_override="search_functions")(search_functions),
            function_tool(name_override="get_function_details")(get_function_details),
        ]

        # filter according to disclosure
        return [t for t in all_tools if disclosure.contains_tool(t.name)]

    def pydantic_ai_tools(
        self,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[PydanticAITool]":
        """
        Expose PCTX tools as Pydantic AI tools

        Args:
            disclosure: Controls which tools are exposed and how function context is
                provided to the model. CATALOG (default) exposes list_functions,
                get_function_details, and execute_typescript — the agent discovers
                and retrieves function signatures before executing. FS exposes
                execute_bash and execute_typescript — the agent browses the virtual
                filesystem directly.
            descriptions: Optional custom descriptions to override defaults.

        Requires the 'pydantic-ai' extra to be installed:
            pip install pctx[pydantic-ai]

        Raises:
            ImportError: If pydantic-ai is not installed.

        Examples:
            >>> tools = pctx.pydantic_ai_tools()  # default: catalog
            >>> tools = pctx.pydantic_ai_tools(disclosure="filesystem")
            >>> tools = pctx.pydantic_ai_tools(descriptions={"execute_typescript": "Custom"})
        """
        disclosure = ToolDisclosure(disclosure)
        try:
            from pydantic_ai.tools import Tool as PydanticAITool
        except ImportError as e:
            raise ImportError(
                "Pydantic AI is not installed. Install it with: pip install pctx[pydantic-ai]"
            ) from e

        # build all tools

        async def execute_bash(command: str) -> str:
            return (await self.execute_bash(command)).markdown()

        async def execute_typescript(code: str) -> str:
            return (
                await self.execute_typescript(code, disclosure=disclosure)
            ).markdown()

        async def list_functions() -> str:
            return (await self.list_functions()).code

        async def search_functions(query: str, k: int = 10) -> str:
            return self._search_functions_result_to_string(
                await self.search_functions(query, k)
            )

        async def get_function_details(functions: list[str]) -> str:
            return (await self.get_function_details(functions)).code

        all_tools = [
            PydanticAITool(
                execute_bash,
                name="execute_bash",
                description=get_tool_description(
                    "execute_bash", overrides=descriptions
                ),
            ),
            PydanticAITool(
                execute_typescript,
                name="execute_typescript",
                description=get_tool_description(
                    "execute_typescript", disclosure=disclosure, overrides=descriptions
                ),
            ),
            PydanticAITool(
                list_functions,
                name="list_functions",
                description=get_tool_description(
                    "list_functions", overrides=descriptions
                ),
            ),
            PydanticAITool(
                search_functions,
                name="search_functions",
                description=get_tool_description(
                    "search_functions", overrides=descriptions
                ),
            ),
            PydanticAITool(
                get_function_details,
                name="get_function_details",
                description=get_tool_description(
                    "get_function_details", overrides=descriptions
                ),
            ),
        ]

        # filter according to disclosure
        return [t for t in all_tools if disclosure.contains_tool(t.name)]


    def claude_agent_sdk_tools(
        self,
        disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
        descriptions: dict[ToolName, str] | None = None,
    ) -> "list[ClaudeSdkMcpTool]":
        """
        Expose PCTX tools as Claude Agent SDK tools

        Args:
            disclosure: Controls which tools are exposed and how function context is
                provided to the model. CATALOG (default) exposes list_functions,
                get_function_details, and execute_typescript — the agent discovers
                and retrieves function signatures before executing. FS exposes
                execute_bash and execute_typescript — the agent browses the virtual
                filesystem directly.
            descriptions: Optional custom descriptions to override defaults.

        Requires the 'claude' extra to be installed:
            pip install pctx[claude]

        Raises:
            ImportError: If claude is not installed.

        Examples:
            >>> tools = pctx.claude_agent_sdk_tools()  # default: catalog
            >>> tools = pctx.claude_agent_sdk_tools(disclosure="filesystem")
            >>> tools = pctx.claude_agent_sdk_tools(descriptions={"execute_typescript": "Custom"})
        """
        disclosure = ToolDisclosure(disclosure)
        try:
            from claude_agent_sdk import tool as claude_tool
        except ImportError as e:
            raise ImportError(
                "Claude Agent SDK is not installed. Install it with: pip install pctx[claude]"
            ) from e
        
        def _text_content_block(val: str) -> dict:
            return {"content": [{"type": "text", "text": val}]}

        # build all tools
        @claude_tool("execute_bash", get_tool_description("execute_bash", overrides=descriptions), ExecuteBashInput.model_json_schema())
        async def execute_bash(args: dict[str, Any]) -> str:
            tool_input = ExecuteBashInput(**args)
            bash_out = await self.execute_bash(tool_input.command)
            return _text_content_block(bash_out.markdown())

        @claude_tool("execute_typescript", get_tool_description("execute_typescript", disclosure=disclosure, overrides=descriptions), ExecuteTypescriptInput.model_json_schema())
        async def execute_typescript(args: dict[str, Any]) -> str:
            tool_input = ExecuteTypescriptInput(**args)
            exec_out = await self.execute_typescript(tool_input.code, disclosure=disclosure)
            return _text_content_block(exec_out.markdown())

        @claude_tool("list_functions", get_tool_description("list_functions", overrides=descriptions), {"type": "object"})
        async def list_functions(_args: dict[str, Any]) -> str:
            listed = await self.list_functions()
            return _text_content_block(listed.code)
        
        @claude_tool("get_function_details", get_tool_description("get_function_details", overrides=descriptions), GetFunctionDetailsInput.model_json_schema())
        async def get_function_details(args: dict[str, Any]) -> str:
            tool_input = GetFunctionDetailsInput(**args)
            details = await self.get_function_details(tool_input.functions)
            return _text_content_block(details.code)
        
        class SearchFunctionsInput(BaseModel):
            query: str
            k: int = 10
        
        @claude_tool("search_functions", get_tool_description("search_functions", overrides=descriptions), SearchFunctionsInput.model_json_schema())
        async def search_functions(args: dict[str, Any]) -> str:
            print(f"Claude fn called search_functions: {args}")
            tool_input = SearchFunctionsInput(**args)
            functions = await self.search_functions(tool_input.query, tool_input.k)
            return _text_content_block(self._search_functions_result_to_string(functions))

        all_tools = [
            execute_bash,
            execute_typescript,
            list_functions,
            search_functions,
            get_function_details,
        ]

        # filter according to disclosure
        return [t for t in all_tools if disclosure.contains_tool(t.name)]
