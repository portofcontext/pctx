"""Integration tests for pctx code mode against a running server"""

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

        @tool("foo_bar", namespace="namespaced_with_underscore")
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

            output = await pctx.execute(code)

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

            output = await pctx.execute(code)

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
            output1 = await pctx.execute(code1)
            assert output1.success, "First execution should succeed"
            assert output1.output is not None, "output1 should have output"
            assert output1.output.get("execution") == 1

            # Second execution - variables don't persist between runs
            code2 = """
            async function run() {
                return { execution: 2, value: 200 };
            }
            """
            output2 = await pctx.execute(code2)
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

            output = await pctx.execute(code)
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

        async with Pctx(tools=[add_numbers, greet, now_timestamp]) as pctx:
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

            # Test calling the add_numbers tool
            code = """
            async function run() {
                const sum = await Tools.addNumbers({ a: 10, b: 32 });
                console.log("Addition result:", sum);
                const now = await Tools.nowTimestamp(null);
                console.log("Now result:", now);
                return { sum, now };
            }
            """
            output = await pctx.execute(code)

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
            output2 = await pctx.execute(code2)

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
            output3 = await pctx.execute(code3)

            assert output3.success, "Third execution should succeed"
            assert output3.output is not None, "output3 should have output"
            assert output3.output.get("greeting") == "Hi, Alice!", (
                "Expected greeting to be 'Hi, Alice!'"
            )

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
            output = await pctx.execute(code)

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
            output = await pctx.execute(code)

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
            output = await pctx.execute(code)

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
            output = await pctx.execute(code)

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
