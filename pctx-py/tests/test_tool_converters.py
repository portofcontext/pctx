"""
Tests for tool converter methods

These tests use the actual framework packages to ensure conversions work correctly.
All optional dependencies are assumed to be installed in the test environment.
"""

import inspect

import pytest

# Import the actual frameworks we're testing against
from crewai.tools import BaseTool as CrewAIBaseTool
from pydantic_ai.tools import Tool as PydanticAITool

from pctx_client import Pctx


@pytest.fixture
def pctx_client():
    """Create a PCTX client instance for testing"""
    return Pctx(tools=[], url="http://localhost:8080")


# ============== LangChain Tests ==============


class TestLangChainConverter:
    """Tests for LangChain tool converter"""

    def test_langchain_tools_returns_list(self, pctx_client):
        """Test that langchain_tools returns a list of LangChain tools"""
        tools = pctx_client.langchain_tools()
        assert isinstance(tools, list)
        assert len(tools) == 4

    def test_langchain_tools_are_langchain_tools(self, pctx_client):
        """Test that all tools are actually LangChain BaseTool instances"""
        tools = pctx_client.langchain_tools()
        for tool in tools:
            # LangChain tools created with @tool decorator are structured tools
            assert hasattr(tool, "name")
            assert hasattr(tool, "description")
            # LangChain tools are invokable (have invoke/ainvoke methods)
            assert hasattr(tool, "invoke") or hasattr(tool, "ainvoke")

    def test_langchain_tool_names(self, pctx_client):
        """Test that LangChain tools have the correct names"""
        tools = pctx_client.langchain_tools()
        names = [tool.name for tool in tools]
        assert "list_functions" in names
        assert "search_functions" in names
        assert "get_function_details" in names
        assert "execute" in names

    def test_langchain_tool_descriptions(self, pctx_client):
        """Test that LangChain tools have descriptions"""
        tools = pctx_client.langchain_tools()
        for tool in tools:
            assert tool.description
            assert len(tool.description) > 0

    def test_langchain_tools_are_async(self, pctx_client):
        """Test that LangChain tools are async callables"""
        tools = pctx_client.langchain_tools()
        for tool in tools:
            # LangChain tools should be coroutine functions
            # We need to check the underlying coroutine function
            assert inspect.iscoroutinefunction(
                tool.invoke
            ) or inspect.iscoroutinefunction(tool.ainvoke)


# ============== CrewAI Tests ==============


class TestCrewAIConverter:
    """Tests for CrewAI tool converter"""

    def test_crewai_tools_returns_list(self, pctx_client):
        """Test that c() returns a list of CrewAI tools"""
        tools = pctx_client.crewai_tools()
        assert isinstance(tools, list)
        assert len(tools) == 4

    def test_crewai_tools_are_crewai_basetools(self, pctx_client):
        """Test that all tools are CrewAI BaseTool instances"""
        tools = pctx_client.crewai_tools()
        for tool in tools:
            assert isinstance(tool, CrewAIBaseTool)

    def test_crewai_tool_names(self, pctx_client):
        """Test that CrewAI tools have correct names"""
        tools = pctx_client.crewai_tools()
        names = [tool.name for tool in tools]
        assert "list_functions" in names
        assert "search_functions" in names
        assert "get_function_details" in names
        assert "execute" in names

    def test_crewai_tool_descriptions(self, pctx_client):
        """Test that CrewAI tools have descriptions"""
        tools = pctx_client.crewai_tools()
        for tool in tools:
            assert tool.description
            assert len(tool.description) > 0

    def test_crewai_tools_have_run_method(self, pctx_client):
        """Test that CrewAI tools have the _run method"""
        tools = pctx_client.crewai_tools()
        for tool in tools:
            assert hasattr(tool, "_run")
            assert callable(tool._run)

    def test_crewai_search_functions_has_schema(self, pctx_client):
        """Test that search_functions tool has args_schema"""
        tools = pctx_client.crewai_tools()
        search_tool = next(t for t in tools if t.name == "search_functions")
        assert hasattr(search_tool, "args_schema")
        assert search_tool.args_schema is not None

    def test_crewai_get_function_details_has_schema(self, pctx_client):
        """Test that get_function_details tool has args_schema"""
        tools = pctx_client.crewai_tools()
        get_details_tool = next(t for t in tools if t.name == "get_function_details")
        assert hasattr(get_details_tool, "args_schema")
        assert get_details_tool.args_schema is not None

    def test_crewai_execute_has_schema(self, pctx_client):
        """Test that execute tool has args_schema"""
        tools = pctx_client.crewai_tools()
        execute_tool = next(t for t in tools if t.name == "execute")
        assert hasattr(execute_tool, "args_schema")
        assert execute_tool.args_schema is not None


# ============== OpenAI Agents SDK Tests ==============


class TestOpenAIAgentsConverter:
    """Tests for OpenAI Agents SDK tool converter"""

    def test_openai_agents_tools_returns_list(self, pctx_client):
        """Test that openai_agents_tools returns a list"""
        tools = pctx_client.openai_agents_tools()
        assert isinstance(tools, list)
        assert len(tools) == 4

    def test_openai_agents_tools_structure(self, pctx_client):
        """Test that OpenAI Agents tools have correct structure"""
        from agents import FunctionTool

        tools = pctx_client.openai_agents_tools()
        for tool in tools:
            assert isinstance(tool, FunctionTool)
            assert hasattr(tool, "name")
            assert hasattr(tool, "description")
            assert hasattr(tool, "params_json_schema")

    def test_openai_agents_function_names(self, pctx_client):
        """Test that OpenAI Agents functions have correct names"""
        tools = pctx_client.openai_agents_tools()
        names = [tool.name for tool in tools]
        assert "list_functions" in names
        assert "search_functions" in names
        assert "get_function_details" in names
        assert "execute" in names

    def test_openai_agents_function_descriptions(self, pctx_client):
        """Test that OpenAI Agents functions have descriptions"""
        tools = pctx_client.openai_agents_tools()
        for tool in tools:
            description = tool.description
            assert description
            assert len(description) > 0

    def test_openai_agents_parameters_schema(self, pctx_client):
        """Test that OpenAI Agents tools have correct parameter schemas"""
        tools = pctx_client.openai_agents_tools()
        for tool in tools:
            params = tool.params_json_schema
            assert params["type"] == "object"
            assert "properties" in params
            assert "required" in params

    def test_openai_agents_search_functions_schema(self, pctx_client):
        """Test search_functions has correct schema"""
        tools = pctx_client.openai_agents_tools()
        search_tool = next(t for t in tools if t.name == "search_functions")
        params = search_tool.params_json_schema
        assert "query" in params["properties"]
        assert "k" in params["properties"]
        assert params["properties"]["query"]["type"] == "string"
        assert params["properties"]["k"]["type"] == "integer"
        assert "query" in params["required"]
        assert "k" in params["required"]

    def test_openai_agents_get_function_details_schema(self, pctx_client):
        """Test get_function_details has correct schema"""
        tools = pctx_client.openai_agents_tools()
        get_details_tool = next(t for t in tools if t.name == "get_function_details")
        params = get_details_tool.params_json_schema
        assert "functions" in params["properties"]
        assert params["properties"]["functions"]["type"] == "array"
        assert "functions" in params["required"]

    def test_openai_agents_execute_schema(self, pctx_client):
        """Test execute has correct schema"""
        tools = pctx_client.openai_agents_tools()
        execute_tool = next(t for t in tools if t.name == "execute")
        params = execute_tool.params_json_schema
        assert "code" in params["properties"]
        assert params["properties"]["code"]["type"] == "string"
        assert "code" in params["required"]


# ============== Pydantic AI Tests ==============


class TestPydanticAIConverter:
    """Tests for Pydantic AI tool converter"""

    def test_pydantic_ai_tools_returns_list(self, pctx_client):
        """Test that pydantic_ai_tools returns a list"""
        tools = pctx_client.pydantic_ai_tools()
        assert isinstance(tools, list)
        assert len(tools) == 4

    def test_pydantic_ai_tools_are_pydantic_ai_tools(self, pctx_client):
        """Test that all tools are Pydantic AI Tool instances"""
        tools = pctx_client.pydantic_ai_tools()
        for tool in tools:
            assert isinstance(tool, PydanticAITool)

    def test_pydantic_ai_tool_names(self, pctx_client):
        """Test that Pydantic AI tools have correct names"""
        tools = pctx_client.pydantic_ai_tools()
        names = [tool.name for tool in tools]
        assert "list_functions" in names
        assert "search_functions" in names
        assert "get_function_details" in names
        assert "execute" in names

    def test_pydantic_ai_tool_descriptions(self, pctx_client):
        """Test that Pydantic AI tools have descriptions"""
        tools = pctx_client.pydantic_ai_tools()
        for tool in tools:
            assert tool.description
            assert len(tool.description) > 0

    def test_pydantic_ai_tools_have_function(self, pctx_client):
        """Test that Pydantic AI tools have callable functions"""
        tools = pctx_client.pydantic_ai_tools()
        for tool in tools:
            assert hasattr(tool, "function")
            assert callable(tool.function)

    def test_pydantic_ai_tools_are_async(self, pctx_client):
        """Test that Pydantic AI tool functions are async"""
        tools = pctx_client.pydantic_ai_tools()
        for tool in tools:
            assert inspect.iscoroutinefunction(tool.function)


# ============== Integration Tests ==============


class TestConverterIntegration:
    """Integration tests to ensure all converters work together"""

    def test_all_converters_available(self, pctx_client):
        """Test that all converter methods are available on Pctx instance"""
        assert hasattr(pctx_client, "langchain_tools")
        assert hasattr(pctx_client, "crewai_tools")
        assert hasattr(pctx_client, "openai_agents_tools")
        assert hasattr(pctx_client, "pydantic_ai_tools")

    def test_converter_methods_callable(self, pctx_client):
        """Test that all converter methods are callable"""
        assert callable(pctx_client.langchain_tools)
        assert callable(pctx_client.crewai_tools)
        assert callable(pctx_client.openai_agents_tools)
        assert callable(pctx_client.pydantic_ai_tools)

    def test_all_converters_return_three_tools(self, pctx_client):
        """Test that converters return the expected number of tools"""
        # Most converters return 3 tools (one per function)
        assert len(pctx_client.langchain_tools()) == 4
        assert len(pctx_client.crewai_tools()) == 4
        assert len(pctx_client.openai_agents_tools()) == 4
        assert len(pctx_client.pydantic_ai_tools()) == 4

    def test_all_converters_have_same_function_names(self, pctx_client):
        """Test that all converters expose the same three function names"""
        expected_names = {
            "list_functions",
            "search_functions",
            "get_function_details",
            "execute",
        }

        # LangChain
        langchain_names = {tool.name for tool in pctx_client.langchain_tools()}
        assert langchain_names == expected_names

        # CrewAI
        crewai_names = {tool.name for tool in pctx_client.crewai_tools()}
        assert crewai_names == expected_names

        # OpenAI Agents
        openai_names = {tool.name for tool in pctx_client.openai_agents_tools()}
        assert openai_names == expected_names

        # Pydantic AI
        pydantic_names = {tool.name for tool in pctx_client.pydantic_ai_tools()}
        assert pydantic_names == expected_names


# ============== Filesystem Mode Tests ==============


class TestFilesystemMode:
    """Tests for filesystem mode ("fs") in all converters"""

    def test_langchain_fs_mode_returns_two_tools(self, pctx_client):
        """Test that langchain_tools("fs") returns exactly 2 tools"""
        tools = pctx_client.langchain_tools("fs")
        assert isinstance(tools, list)
        assert len(tools) == 2

    def test_langchain_fs_mode_tool_names(self, pctx_client):
        """Test that langchain_tools("fs") returns execute_bash and execute_typescript"""
        tools = pctx_client.langchain_tools("fs")
        names = {tool.name for tool in tools}
        assert names == {"execute_bash", "execute_typescript"}

    def test_langchain_fs_mode_tool_descriptions(self, pctx_client):
        """Test that fs_mode tools have proper descriptions"""
        tools = pctx_client.langchain_tools("fs")
        for tool in tools:
            assert tool.description
            assert len(tool.description) > 0
            # Check that descriptions mention filesystem/SDK exploration
            if tool.name == "execute_bash":
                assert (
                    "filesystem" in tool.description.lower()
                    or "bash" in tool.description.lower()
                )
            elif tool.name == "execute_typescript":
                assert (
                    "typescript" in tool.description.lower()
                    or "code" in tool.description.lower()
                )

    def test_crewai_fs_mode_returns_two_tools(self, pctx_client):
        """Test that crewai_tools("fs") returns exactly 2 tools"""
        tools = pctx_client.crewai_tools("fs")
        assert isinstance(tools, list)
        assert len(tools) == 2

    def test_crewai_fs_mode_tool_names(self, pctx_client):
        """Test that crewai_tools("fs") returns execute_bash and execute_typescript"""
        tools = pctx_client.crewai_tools("fs")
        names = {tool.name for tool in tools}
        assert names == {"execute_bash", "execute_typescript"}

    def test_crewai_fs_mode_tools_are_basetools(self, pctx_client):
        """Test that fs_mode CrewAI tools are still BaseTool instances"""
        tools = pctx_client.crewai_tools("fs")
        for tool in tools:
            assert isinstance(tool, CrewAIBaseTool)

    def test_crewai_fs_mode_tools_have_schemas(self, pctx_client):
        """Test that fs_mode CrewAI tools have args_schema"""
        tools = pctx_client.crewai_tools("fs")
        for tool in tools:
            assert hasattr(tool, "args_schema")
            assert tool.args_schema is not None

    def test_openai_agents_fs_mode_returns_two_tools(self, pctx_client):
        """Test that openai_agents_tools("fs") returns exactly 2 tools"""
        tools = pctx_client.openai_agents_tools("fs")
        assert isinstance(tools, list)
        assert len(tools) == 2

    def test_openai_agents_fs_mode_tool_names(self, pctx_client):
        """Test that openai_agents_tools("fs") returns execute_bash and execute_typescript"""
        from agents import FunctionTool

        tools = pctx_client.openai_agents_tools("fs")
        names = {tool.name for tool in tools}
        assert names == {"execute_bash", "execute_typescript"}
        # Verify they're still FunctionTool instances
        for tool in tools:
            assert isinstance(tool, FunctionTool)

    def test_openai_agents_fs_mode_tool_schemas(self, pctx_client):
        """Test that fs_mode OpenAI Agents tools have proper schemas"""
        tools = pctx_client.openai_agents_tools("fs")
        for tool in tools:
            params = tool.params_json_schema
            assert params["type"] == "object"
            assert "properties" in params
            assert "required" in params
            # execute_bash should have 'command' parameter
            if tool.name == "execute_bash":
                assert "command" in params["properties"]
                assert "command" in params["required"]
            # execute_typescript should have 'code' parameter
            elif tool.name == "execute_typescript":
                assert "code" in params["properties"]
                assert "code" in params["required"]

    def test_pydantic_ai_fs_mode_returns_two_tools(self, pctx_client):
        """Test that pydantic_ai_tools("fs") returns exactly 2 tools"""
        tools = pctx_client.pydantic_ai_tools("fs")
        assert isinstance(tools, list)
        assert len(tools) == 2

    def test_pydantic_ai_fs_mode_tool_names(self, pctx_client):
        """Test that pydantic_ai_tools("fs") returns execute_bash and execute_typescript"""
        tools = pctx_client.pydantic_ai_tools("fs")
        names = {tool.name for tool in tools}
        assert names == {"execute_bash", "execute_typescript"}

    def test_pydantic_ai_fs_mode_tools_are_pydantic_ai_tools(self, pctx_client):
        """Test that fs_mode Pydantic AI tools are still Tool instances"""
        tools = pctx_client.pydantic_ai_tools("fs")
        for tool in tools:
            assert isinstance(tool, PydanticAITool)

    def test_all_converters_fs_mode_consistency(self, pctx_client):
        """Test that all converters return the same tool names in fs_mode"""
        expected_names = {"execute_bash", "execute_typescript"}

        # LangChain
        langchain_names = {tool.name for tool in pctx_client.langchain_tools("fs")}
        assert langchain_names == expected_names

        # CrewAI
        crewai_names = {tool.name for tool in pctx_client.crewai_tools("fs")}
        assert crewai_names == expected_names

        # OpenAI Agents
        openai_names = {tool.name for tool in pctx_client.openai_agents_tools("fs")}
        assert openai_names == expected_names

        # Pydantic AI
        pydantic_names = {tool.name for tool in pctx_client.pydantic_ai_tools("fs")}
        assert pydantic_names == expected_names

    def test_fs_mode_false_returns_standard_tools(self, pctx_client):
        """Test that default mode returns standard tools, not fs tools"""
        # Test default mode (list_get_execute)
        tools = pctx_client.langchain_tools()
        names = {tool.name for tool in tools}

        # Should have standard tools
        assert "list_functions" in names
        assert "execute" in names

        # Should NOT have fs tools
        assert "execute_bash" not in names
        assert "execute_typescript" not in names


# ============== Custom Tool Descriptions Tests ==============


class TestCustomToolDescriptions:
    """Tests for custom tool descriptions parameter"""

    def test_langchain_custom_descriptions(self, pctx_client):
        """Test that custom descriptions work with langchain_tools"""
        custom_descriptions = {
            "list_functions": "Custom list description",
            "get_function_details": "Custom details description",
            "execute": "Custom execute description",
        }

        tools = pctx_client.langchain_tools(descriptions=custom_descriptions)

        # Find the execute tool and check its description
        execute_tool = next(t for t in tools if t.name == "execute")
        assert execute_tool.description == "Custom execute description"

    def test_crewai_custom_descriptions(self, pctx_client):
        """Test that custom descriptions work with crewai_tools"""
        custom_descriptions = {
            "list_functions": "Custom list description",
            "get_function_details": "Custom details description",
            "execute": "Custom execute description",
        }

        tools = pctx_client.crewai_tools(descriptions=custom_descriptions)

        # Find the execute tool and check its description (CrewAI wraps it with metadata)
        execute_tool = next(t for t in tools if t.name == "execute")
        assert "Custom execute description" in execute_tool.description

    def test_openai_agents_custom_descriptions(self, pctx_client):
        """Test that custom descriptions work with openai_agents_tools"""
        custom_descriptions = {
            "list_functions": "Custom list description",
            "get_function_details": "Custom details description",
            "execute": "Custom execute description",
        }

        tools = pctx_client.openai_agents_tools(descriptions=custom_descriptions)

        # Find the execute tool and check its description contains custom text
        execute_tool = next(t for t in tools if t.name == "execute")
        assert "Custom execute description" in execute_tool.description

    def test_pydantic_ai_custom_descriptions(self, pctx_client):
        """Test that custom descriptions work with pydantic_ai_tools"""
        custom_descriptions = {
            "list_functions": "Custom list description",
            "get_function_details": "Custom details description",
            "execute": "Custom execute description",
        }

        tools = pctx_client.pydantic_ai_tools(descriptions=custom_descriptions)

        # Find the execute tool and check its description
        execute_tool = next(t for t in tools if t.name == "execute")
        assert execute_tool.description == "Custom execute description"

    def test_langchain_fs_mode_custom_descriptions(self, pctx_client):
        """Test that custom descriptions work with fs mode"""
        custom_descriptions = {
            "execute_bash": "Custom bash description",
            "execute_typescript": "Custom typescript description",
        }

        tools = pctx_client.langchain_tools("fs", descriptions=custom_descriptions)

        # Check both tools have custom descriptions
        bash_tool = next(t for t in tools if t.name == "execute_bash")
        ts_tool = next(t for t in tools if t.name == "execute_typescript")

        assert bash_tool.description == "Custom bash description"
        assert ts_tool.description == "Custom typescript description"

    def test_terminal_style_descriptions_work(self, pctx_client):
        """Test that TERMINAL_STYLE_DESCRIPTIONS can be used"""
        from pctx_client.tool_descriptions import TERMINAL_STYLE_DESCRIPTIONS

        tools = pctx_client.langchain_tools(descriptions=TERMINAL_STYLE_DESCRIPTIONS)

        # Verify tools were created and have the terminal-style descriptions
        execute_tool = next(t for t in tools if t.name == "execute")
        # Terminal style is shorter and mentions "Deno sandbox"
        assert "Deno sandbox" in execute_tool.description
        # Should NOT have prescriptive workflow language
        assert "WORKFLOW" not in execute_tool.description


# ============== Tool Registry Tests ==============


class TestToolRegistry:
    """Tests for the central tool registry"""

    def test_registry_contains_all_tools(self):
        """Test that TOOL_REGISTRY contains all ToolName values"""
        from typing import get_args

        from pctx_client._tool_registry import TOOL_REGISTRY
        from pctx_client.tools import ToolName

        # Get all possible tool names from the ToolName literal
        all_tool_names = set(get_args(ToolName))

        # Verify registry contains exactly these tools
        assert TOOL_REGISTRY == all_tool_names

    def test_registry_validation_runs_on_import(self):
        """Test that registry validation runs automatically on import"""
        # This test verifies that the module can be imported without errors
        # If the registry were incomplete, an error would be raised on import
        from pctx_client import _tool_registry

        # Verify the validation function exists
        assert hasattr(_tool_registry, "validate_registry_completeness")
        assert callable(_tool_registry.validate_registry_completeness)

    def test_unknown_tool_raises_error(self, pctx_client):
        """Test that using an unknown tool name raises a clear error"""
        from pctx_client.tools import ToolConfig

        # Create a ToolConfig with a made-up tool name
        # We need to bypass type checking by using a dict
        invalid_config = ToolConfig(tools=["fake_tool_that_does_not_exist"])  # type: ignore

        # All framework adapters should raise ValueError with helpful message
        with pytest.raises(ValueError, match="Unknown tool"):
            pctx_client.langchain_tools(invalid_config)

        with pytest.raises(ValueError, match="Unknown tool"):
            pctx_client.crewai_tools(invalid_config)

        with pytest.raises(ValueError, match="Unknown tool"):
            pctx_client.openai_agents_tools(invalid_config)

        with pytest.raises(ValueError, match="Unknown tool"):
            pctx_client.pydantic_ai_tools(invalid_config)
