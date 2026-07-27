"""Integration tests for pctx code mode against a running server"""

import asyncio
import threading
import time
from datetime import datetime

import pytest

from pctx_client import Pctx, tool
from pctx_client.exceptions import ConnectionError
from pctx_client.models import ListedFunction


@pytest.mark.integration
@pytest.mark.asyncio
async def test_server_connection():
    """Test that we can connect to a running pctx server"""
    pctx = Pctx()

    try:
        await pctx.connect()
        # If we get here, connection succeeded
        await pctx.disconnect()
    except ConnectionError as e:
        # Provide robust error message if server is not running
        pytest.fail(
            f"Failed to connect to pctx server at http://localhost:8080.\n"
            f"Error: {str(e)}\n\n"
            f"Please ensure the pctx server is running at the default location.\n"
            f"Start the server with: pctx server start\n"
            f"Or run: cargo run --bin pctx -- server start"
        )
    except Exception as e:
        # Catch other unexpected errors
        pytest.fail(
            f"Unexpected error while connecting to pctx server: {str(e)}\n"
            f"Server may be running but not responding correctly."
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_list_functions():
    """Test listing functions from code mode"""
    try:
        async with Pctx() as pctx:
            functions = await pctx.list_functions()

            # Verify the response has the expected structure
            assert hasattr(functions, "code"), "Response should have 'code' attribute"
            assert isinstance(functions.code, str), "Code should be a string"
            assert hasattr(functions, "functions"), (
                "Response should have 'functions' attribute"
            )
            assert isinstance(functions.functions, list), "Functions should be a list"

            # With no MCP servers registered, the list may be empty, which is valid
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_search_functions():
    """Test search functions from code mode"""
    try:
        # Define a simple local tool
        @tool
        def add_numbers(a: int, b: int) -> int:
            """Add two numbers together"""
            return a + b

        @tool
        def greet(name: str, greeting: str = "Hello") -> str:
            """Greet someone with a custom greeting"""
            return f"{greeting}, {name}!"

        @tool(name="foo_bar", namespace="namespaced_with_underscore")
        def namespaced_fn(val: str) -> str:
            return f"Hello {val}"

        async with Pctx(tools=[add_numbers, greet, namespaced_fn]) as pctx:
            functions = await pctx.search_functions("Add numbers together", 3)
            assert isinstance(functions, list), "Result should be a list"
            assert len(functions) == 1
            assert isinstance(functions[0], ListedFunction), (
                "Results should ListedFunction"
            )
            assert functions[0].name == "addNumbers", "Search should match addNumbers"

            functions = await pctx.search_functions("greet user", 3)
            assert isinstance(functions, list), "Result should be a list"
            assert len(functions) == 1
            assert isinstance(functions[0], ListedFunction), (
                "Results should ListedFunction"
            )
            assert functions[0].name == "greet", "Search should match greet"

            # Test k greater than available tools
            functions = await pctx.search_functions("Greet number", 5)
            assert isinstance(functions, list), "Result should be a list"
            assert len(functions) == 2

            # test searching underscore namespace
            functions = await pctx.search_functions("namespaced", 3)
            assert len(functions) == 1
            assert functions[0].name == "fooBar", "Search should match fooBar"

            # test searching underscore fn name
            functions = await pctx.search_functions("bar", 3)
            assert len(functions) == 1
            assert functions[0].name == "fooBar", "Search should match fooBar"

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_execute_simple_code():
    """Test executing simple TypeScript code"""
    try:
        async with Pctx() as pctx:
            # Simple code that doesn't require any MCP tools
            code = """
            async function run() {
                const result = 2 + 2;
                console.log("Calculation result:", result);
                return { sum: result, message: "Hello from code mode!" };
            }
            """

            output = await pctx.execute_typescript(code)

            # Verify execution succeeded
            assert output.success, "Execution should succeed"
            assert output.output is not None, "Execution should return output"
            assert output.output.get("sum") == 4, "Expected sum to be 4"
            assert "message" in output.output, "Expected message in output"

            # Verify logs were captured in stdout
            assert len(output.stdout) > 0, "Should have console.log output in stdout"
            assert "Calculation result" in output.stdout, (
                "Should contain our console.log message"
            )
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_execute_with_error():
    """Test that code execution errors are properly reported"""
    try:
        async with Pctx() as pctx:
            # Code that will throw an error at runtime
            code = """
            async function run(): Promise<any> {
                throw new Error("Intentional test error");
            }
            """

            output = await pctx.execute_typescript(code)

            # When code throws an error, success should be False
            assert not output.success, "Execution should report failure"
            # Error should be in stderr
            assert "Intentional test error" in output.stderr, (
                f"stderr should contain 'Intentional test error'. Got: {output.stderr}"
            )
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_multiple_sequential_executions():
    """Test that we can execute code multiple times in the same session"""
    try:
        async with Pctx() as pctx:
            # First execution
            code1 = """
            async function run() {
                return { execution: 1, value: 100 };
            }
            """
            output1 = await pctx.execute_typescript(code1)
            assert output1.success, "First execution should succeed"
            assert output1.output is not None, "output1 should have output"
            assert output1.output.get("execution") == 1

            # Second execution - variables don't persist between runs
            code2 = """
            async function run() {
                return { execution: 2, value: 200 };
            }
            """
            output2 = await pctx.execute_typescript(code2)
            assert output2.success, "Second execution should succeed"
            assert output2.output is not None, "output2 should have output"
            assert output2.output.get("execution") == 2

            # Verify they're independent
            assert output1.output.get("value") == 100
            assert output2.output.get("value") == 200
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_connection_with_custom_url():
    """Test connection with explicitly specified URL"""
    # Test that default URL works
    pctx = Pctx()  # Uses default http://localhost:8080

    try:
        await pctx.connect()
        await pctx.disconnect()
    except ConnectionError as e:
        pytest.fail(
            f"Failed to connect to pctx server at default location (http://localhost:8080).\n"
            f"Error: {str(e)}\n\n"
            f"Please ensure the pctx server is running.\n"
            f"Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_markdown_output_formatting():
    """Test that markdown output formatting works correctly"""
    try:
        async with Pctx() as pctx:
            code = """
            async function run() {
                console.log("Step 1: Starting");
                console.log("Step 2: Processing");
                return { status: "completed", count: 42 };
            }
            """

            output = await pctx.execute_typescript(code)
            markdown = output.markdown()

            # Verify markdown output contains expected elements
            assert isinstance(markdown, str), "markdown() should return a string"
            assert len(markdown) > 0, "Markdown output should not be empty"

            # Should contain both stdout sections
            assert "Step 1" in markdown, (
                "Markdown should contain 'Step 1' console.log output"
            )
            assert "Step 2" in markdown, (
                "Markdown should contain 'Step 2' console.log output"
            )

            # Should contain output data
            assert "completed" in markdown, (
                "Markdown should contain 'completed' from output"
            )
            assert "42" in markdown, "Markdown should contain '42' from output"
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_local_python_tool_registration_and_calling():
    """Test registering and calling local Python tools"""
    try:
        # Define a simple local tool
        @tool
        def add_numbers(a: int, b: int) -> int:
            """Add two numbers together"""
            return a + b

        @tool
        def greet(name: str, greeting: str = "Hello") -> str:
            """Greet someone with a custom greeting"""
            return f"{greeting}, {name}!"

        @tool
        def now_timestamp() -> float:
            """Returns current timestamp"""
            return datetime.now().timestamp()

        @tool
        def search_logs(
            query: str = "", level: str = "info", limit: int = 100
        ) -> list[dict]:
            """Search application logs with optional filters"""
            return [
                {"message": f"match for '{query}'", "level": level, "index": i}
                for i in range(min(limit, 3))
            ]

        async with Pctx(tools=[add_numbers, greet, now_timestamp, search_logs]) as pctx:
            # Verify tools are listed
            functions = await pctx.list_functions()
            function_names = [f"{f.namespace}.{f.name}" for f in functions.functions]

            assert "Tools.addNumbers" in function_names, (
                f"addNumbers tool should be registered, got: {function_names}"
            )
            assert "Tools.greet" in function_names, (
                f"greet tool should be registered, got: {function_names}"
            )
            assert "Tools.nowTimestamp" in function_names, (
                f"now_timestamp tool should be registered, got: {function_names}"
            )
            assert "Tools.searchLogs" in function_names, (
                f"searchLogs tool should be registered, got: {function_names}"
            )

            # Test calling the add_numbers tool
            code = """
            async function run() {
                const result = await Tools.addNumbers({ a: 10, b: 32 });
                return { sum: result };
            }
            """
            output = await pctx.execute_typescript(code)

            assert output.success, "Execution should succeed"
            assert output.output is not None, "Should have output"
            assert output.output.get("sum") == 42, "Expected sum to be 42"

            # Test calling the greet tool with default parameter
            code2 = """
            async function run() {
                const result = await Tools.greet({ name: "World" });
                return { greeting: result };
            }
            """
            output2 = await pctx.execute_typescript(code2)

            assert output2.success, "Second execution should succeed"
            assert output2.output is not None, "output2 should have output"
            assert output2.output.get("greeting") == "Hello, World!", (
                "Expected greeting to be 'Hello, World!'"
            )

            # Test calling the greet tool with custom greeting
            code3 = """
            async function run() {
                const result = await Tools.greet({ name: "Alice", greeting: "Hi" });
                return { greeting: result };
            }
            """
            output3 = await pctx.execute_typescript(code3)

            assert output3.success, "Third execution should succeed"
            assert output3.output is not None, "output3 should have output"
            assert output3.output.get("greeting") == "Hi, Alice!", (
                "Expected greeting to be 'Hi, Alice!'"
            )

            # Test calling the now_timestamp tool
            code4 = """
            async function run() {
                const result = await Tools.nowTimestamp();
                return { timestamp: result };
            }
            """
            output4 = await pctx.execute_typescript(code4)
            assert output4.success, "Fourth execution should succeed"
            assert output4.output is not None, "output4 should have output"
            assert isinstance(output4.output.get("timestamp"), float), (
                "Expected timestamp to be a float"
            )

            # Test calling search_logs - all optional params with defaults, and with explicit filters
            code5 = """
            async function run() {
                const noInput = await Tools.searchLogs();
                const empty = await Tools.searchLogs({});
                const filtered = await Tools.searchLogs({ query: "error", level: "error", limit: 1 });
                return { noInput, empty, filtered };
            }
            """
            output5 = await pctx.execute_typescript(code5)

            assert output5.success, (
                f"search_logs should succeed. stderr: {output5.stderr}"
            )
            assert output5.output is not None, "output5 should have output"

            for return_attr in ["noInput", "empty"]:
                val = output5.output.get(return_attr)
                assert len(val) == 3, (
                    f"Expected 3 log entries with {return_attr}, got {len(val)}"
                )
                assert val[0].get("level") == "info", "Expected default level 'info'"

            filtered = output5.output.get("filtered")
            assert len(filtered) == 1, (
                f"Expected 1 log entry with limit=1, got {len(filtered)}"
            )
            assert filtered[0].get("level") == "error", "Expected level 'error'"
            assert "error" in filtered[0].get("message"), "Expected query in message"

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_async_local_python_tool():
    """Test registering and calling async local Python tools"""
    try:
        # Define an async local tool
        @tool
        async def fetch_data(item_id: int) -> dict:
            """Simulate fetching data asynchronously"""
            # Simulate some async work
            import asyncio

            await asyncio.sleep(0.01)
            return {"id": item_id, "status": "fetched", "data": f"Item {item_id}"}

        async with Pctx(tools=[fetch_data]) as pctx:
            # Verify tool is listed
            functions = await pctx.list_functions()
            function_names = [f"{f.namespace}.{f.name}" for f in functions.functions]

            assert "Tools.fetchData" in function_names, (
                f"fetchData tool should be registered, got: {function_names}"
            )

            # Test calling the async tool
            code = """
            async function run() {
                const result = await Tools.fetchData({ item_id: 123 });
                return result;
            }
            """
            output = await pctx.execute_typescript(code)

            assert output.success, "Execution should succeed"
            assert output.output is not None, "Should have output"
            assert output.output.get("id") == 123, "Expected id to be 123"
            assert output.output.get("status") == "fetched", (
                "Expected status to be 'fetched'"
            )

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_http_mcp_server_registration(http_mcp_server):
    """Test registering and using an HTTP MCP server"""
    # http_mcp_server fixture ensures the server is running
    try:
        from pctx_client import HttpServerConfig

        http_server: HttpServerConfig = {
            "name": "test_http_mcp",
            "url": "http://localhost:8765/mcp",
        }

        async with Pctx(servers=[http_server]) as pctx:
            # List functions to see if HTTP MCP server functions are available
            functions = await pctx.list_functions()

            assert isinstance(functions.functions, list), (
                "Should return a list of functions"
            )

            # Check if HTTP server functions are available
            http_functions = [
                f for f in functions.functions if f.namespace == "TestHttpMcp"
            ]

            assert len(http_functions) > 0, (
                f"Expected HTTP MCP functions from test_http_mcp server. "
                f"Got functions: {[f'{f.namespace}.{f.name}' for f in functions.functions]}"
            )

            # Verify we have the expected functions from our test server
            function_names = {f.name for f in http_functions}
            expected_functions = {"subtract", "divide", "concat", "reverseString"}
            assert expected_functions.issubset(function_names), (
                f"Expected functions {expected_functions}, got {function_names}"
            )

            # Test calling one of the HTTP MCP server functions
            code = """
            async function run() {
                const result = await TestHttpMcp.subtract({ a: 50, b: 8 });
                console.log("HTTP MCP subtract result:", JSON.stringify(result));
                return { difference: result };
            }
            """
            output = await pctx.execute_typescript(code)

            assert output.success, f"Execution should succeed. stderr: {output.stderr}"
            assert output.output is not None, "Should have output"

            # HTTP MCP tools return wrapped in result object
            assert output.output.get("difference").get("result") == 42, (
                f"Expected difference to be 42, got {output.output.get('difference')}"
            )

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_stdio_mcp_server_registration():
    """Test registering and using a stdio MCP server"""
    try:
        import os
        import sys

        from pctx_client import StdioServerConfig

        # Get the path to the test MCP server script
        test_script = os.path.join(
            os.path.dirname(__file__), "scripts", "test_mcp_server.py"
        )

        # Use our Python test MCP server
        # Use sys.executable to ensure we use the same Python interpreter
        # that's running the tests (to work in both local dev and CI)
        stdio_server: StdioServerConfig = {
            "name": "TestMcpServer",
            "command": sys.executable,
            "args": [test_script],
        }

        async with Pctx(servers=[stdio_server]) as pctx:
            # List functions to see if stdio MCP server functions are available
            functions = await pctx.list_functions()

            assert isinstance(functions.functions, list), (
                "Should return a list of functions"
            )

            # Check if stdio server functions are available
            stdio_functions = [
                f for f in functions.functions if f.namespace == "TestMcpServer"
            ]

            assert len(stdio_functions) > 0, (
                f"Expected stdio MCP functions, got: {[f'{f.namespace}.{f.name}' for f in functions.functions]}"
            )

            # Verify we have the expected functions from our test server
            function_names = {f.name for f in stdio_functions}
            expected_functions = {"add", "multiply", "greet", "echo"}
            assert expected_functions.issubset(function_names), (
                f"Expected functions {expected_functions}, got {function_names}"
            )

            # Test calling one of the stdio MCP server functions
            code = """
            async function run() {
                const result = await TestMcpServer.add({ a: 15, b: 27 });
                console.log("MCP add result:", JSON.stringify(result));
                return { sum: result };
            }
            """
            output = await pctx.execute_typescript(code)

            assert output.success, f"Execution should succeed. stderr: {output.stderr}"
            assert output.output is not None, "Should have output"

            # MCP tools return wrapped in result object
            assert output.output.get("sum").get("result") == 42, (
                f"Expected sum to be 42, got {output.output.get('sum')}"
            )

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_execute_bash_virtual_filesystem():
    """Test executing bash commands in the virtual filesystem"""
    try:
        # Define tools to populate the virtual filesystem
        @tool
        def add_numbers(a: int, b: int) -> int:
            """Add two numbers together"""
            return a + b

        @tool
        def greet(name: str) -> str:
            """Greet someone"""
            return f"Hello, {name}!"

        async with Pctx(tools=[add_numbers, greet]) as pctx:
            # Test 1: List files in SDK directory (cwd is /sdk/)
            output = await pctx.execute_bash("ls")
            print(output.markdown())
            assert output.exit_code == 0, "ls command should succeed"
            assert "README.md" in output.stdout, "Should have README.md"
            assert "Tools" in output.stdout, "Should have Tools namespace folder"
            assert "bin" not in output.stdout, "Should NOT have system bin dir"
            assert "proc" not in output.stdout, "Should NOT have system proc dir"

            # Test 2: Read the README
            output = await pctx.execute_bash("cat README.md")
            print(output.markdown())
            assert output.exit_code == 0, "cat command should succeed"
            assert "TypeScript SDK" in output.stdout, "README should have header"
            assert "## Tools" in output.stdout, "README should list Tools namespace"
            assert "addNumbers" in output.stdout, (
                "README should list addNumbers function"
            )
            assert "greet" in output.stdout, "README should list greet function"

            # Test 3: Grep for specific content
            output = await pctx.execute_bash("grep 'add two numbers' README.md")
            assert output.exit_code == 0, "grep command should succeed"
            assert "add two numbers" in output.stdout, (
                "Should find the addNumbers description"
            )

            # Test 4: List files in Tools namespace directory
            output = await pctx.execute_bash("ls Tools/")
            assert output.exit_code == 0, "Should list Tools namespace directory"
            assert "addNumbers.d.ts" in output.stdout, (
                "Should have addNumbers.d.ts file"
            )
            assert "greet.d.ts" in output.stdout, "Should have greet.d.ts file"

            # Test 5: Read individual function TypeScript definition file
            output = await pctx.execute_bash("cat Tools/addNumbers.d.ts")
            assert output.exit_code == 0, "Should read TypeScript definitions"
            assert "function addNumbers" in output.stdout, (
                "Should have addNumbers function signature"
            )
            assert "a: number" in output.stdout, (
                "Should have typed parameters in signatures"
            )

            # Test 6: Test command that should fail
            output = await pctx.execute_bash("cat nonexistent.txt")
            assert not output.exit_code == 0, "Should fail for nonexistent file"
            assert len(output.stderr) > 0, "Should have error message in stderr"

            # Test 7: Complex pipe command to find function files
            output = await pctx.execute_bash("ls Tools/ | grep '.d.ts'")
            assert output.exit_code == 0, "Pipe command should succeed"
            assert "addNumbers.d.ts" in output.stdout, "Should find .d.ts files"
            assert "greet.d.ts" in output.stdout, "Should find .d.ts files"

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_bash_then_typescript_workflow():
    """Test using bash to explore, then TypeScript to execute"""
    try:

        @tool
        def multiply(x: int, y: int) -> int:
            """Multiply two numbers"""
            return x * y

        async with Pctx(tools=[multiply]) as pctx:
            # Step 1: Use bash to discover available functions (cwd is /sdk/)
            output = await pctx.execute_bash("cat README.md")
            assert output.exit_code == 0, "Should read README"
            assert "multiply" in output.stdout, "Should find multiply function"

            # Step 2: Read the individual function TypeScript definition to understand the signature
            output = await pctx.execute_bash("cat Tools/multiply.d.ts")
            assert output.exit_code == 0, "Should read TypeScript definitions"
            assert "function multiply" in output.stdout, (
                "Should have multiply signature"
            )

            # Step 3: Use TypeScript to call the function we discovered
            code = """
            async function run() {
                const result = await Tools.multiply({ x: 6, y: 7 });
                return { product: result };
            }
            """
            output = await pctx.execute_typescript(code)
            assert output.success, "TypeScript execution should succeed"
            assert output.output is not None, "Should have output"
            assert output.output.get("product") == 42, "Expected product to be 42"

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_mixed_tools_and_mcp_servers():
    """Test using local tools alongside MCP servers"""
    try:
        # Define local tools
        @tool
        def multiply(x: int, y: int) -> int:
            """Multiply two numbers"""
            return x * y

        @tool
        def format_result(value: int, label: str) -> str:
            """Format a result with a label"""
            return f"{label}: {value}"

        # Note: MCP servers would be added here if available
        async with Pctx(tools=[multiply, format_result]) as pctx:
            # Verify all tools are listed
            functions = await pctx.list_functions()
            function_names = [f"{f.namespace}.{f.name}" for f in functions.functions]

            assert "Tools.multiply" in function_names, (
                f"multiply tool should be registered, got: {function_names}"
            )
            assert "Tools.formatResult" in function_names, (
                f"formatResult tool should be registered, got: {function_names}"
            )

            # Test calling multiple local tools in sequence
            code = """
            async function run() {
                const product = await Tools.multiply({ x: 6, y: 7 });
                const formatted = await Tools.formatResult({
                    value: product,
                    label: "Result"
                });
                return { product, formatted };
            }
            """
            output = await pctx.execute_typescript(code)

            assert output.success, "Execution should succeed"
            assert output.output is not None, "output should have output"
            assert output.output.get("product") == 42, "Expected product to be 42"
            assert output.output.get("formatted") == "Result: 42", (
                "Expected formatted string"
            )

    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_concurrent_async_tool_calls_run_in_parallel():
    """Tools fanned out with `Promise.all` must execute concurrently.

    The client used to await each tool request inside its WebSocket read loop,
    so a batch of N calls took the sum of their durations instead of the
    slowest one. Assert on observed overlap rather than wall time alone, so
    the test fails on serialization rather than on a slow machine.
    """
    sleep_secs = 0.5
    calls = 4

    in_flight = 0
    max_in_flight = 0

    @tool
    async def slow_echo(value: int) -> int:
        """Sleep briefly, then echo the value back"""
        nonlocal in_flight, max_in_flight
        in_flight += 1
        max_in_flight = max(max_in_flight, in_flight)
        try:
            await asyncio.sleep(sleep_secs)
            return value
        finally:
            in_flight -= 1

    try:
        async with Pctx(tools=[slow_echo], execute_timeout=60) as pctx:
            code = """
            async function run() {
                const values = await Promise.all([
                    Tools.slowEcho({ value: 1 }),
                    Tools.slowEcho({ value: 2 }),
                    Tools.slowEcho({ value: 3 }),
                    Tools.slowEcho({ value: 4 }),
                ]);
                return { values };
            }
            """

            start = time.perf_counter()
            output = await pctx.execute_typescript(code)
            elapsed = time.perf_counter() - start

            assert output.success, f"Execution should succeed, got: {output.stderr}"
            assert output.output is not None, "Execution should return output"
            assert output.output.get("values") == [1, 2, 3, 4], (
                f"Expected all four calls to return, got: {output.output}"
            )

            assert max_in_flight == calls, (
                f"All {calls} tool calls should be in flight at once, "
                f"peaked at {max_in_flight} -- the client is serializing them"
            )

            # Serialized dispatch takes calls * sleep_secs; concurrent dispatch
            # takes ~sleep_secs. Half way between the two is a wide enough
            # margin to absorb session setup and round-trip overhead.
            serial_secs = calls * sleep_secs
            assert elapsed < serial_secs / 2, (
                f"Concurrent calls took {elapsed:.2f}s; serialized dispatch "
                f"would take ~{serial_secs:.2f}s"
            )
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )


@pytest.mark.integration
@pytest.mark.asyncio
async def test_concurrent_sync_tool_calls_run_in_parallel():
    """Sync tools fanned out with `Promise.all` must also run concurrently.

    A sync tool body blocks whatever thread it runs on, so calling it inline
    on the event loop would stall every other in-flight call behind it. They
    run on worker threads instead -- hence the blocking `time.sleep` here, and
    the lock around the counters, which the tool bodies touch off-thread.
    """
    sleep_secs = 0.5
    calls = 4

    lock = threading.Lock()
    in_flight = 0
    max_in_flight = 0

    @tool
    def slow_echo_sync(value: int) -> int:
        """Block briefly, then echo the value back"""
        nonlocal in_flight, max_in_flight
        with lock:
            in_flight += 1
            max_in_flight = max(max_in_flight, in_flight)
        try:
            time.sleep(sleep_secs)
            return value
        finally:
            with lock:
                in_flight -= 1

    try:
        async with Pctx(tools=[slow_echo_sync], execute_timeout=60) as pctx:
            code = """
            async function run() {
                const values = await Promise.all([
                    Tools.slowEchoSync({ value: 1 }),
                    Tools.slowEchoSync({ value: 2 }),
                    Tools.slowEchoSync({ value: 3 }),
                    Tools.slowEchoSync({ value: 4 }),
                ]);
                return { values };
            }
            """

            start = time.perf_counter()
            output = await pctx.execute_typescript(code)
            elapsed = time.perf_counter() - start

            assert output.success, f"Execution should succeed, got: {output.stderr}"
            assert output.output is not None, "Execution should return output"
            assert output.output.get("values") == [1, 2, 3, 4], (
                f"Expected all four calls to return, got: {output.output}"
            )

            assert max_in_flight == calls, (
                f"All {calls} tool calls should be in flight at once, "
                f"peaked at {max_in_flight} -- sync tools are blocking the "
                f"event loop instead of running on worker threads"
            )

            serial_secs = calls * sleep_secs
            assert elapsed < serial_secs / 2, (
                f"Concurrent calls took {elapsed:.2f}s; serialized dispatch "
                f"would take ~{serial_secs:.2f}s"
            )
    except ConnectionError:
        pytest.fail(
            "Failed to connect to pctx server at http://localhost:8080.\n"
            "Please ensure the pctx server is running.\n"
            "Start the server with: pctx server start"
        )
