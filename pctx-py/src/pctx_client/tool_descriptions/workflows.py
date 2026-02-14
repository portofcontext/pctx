"""Workflow system prompts for different tool combinations."""

from ..tools import ToolName


def get_workflow_prompt(tools: list[ToolName]) -> str | None:
    """
    Generate workflow guidance based on available tools.

    Returns system prompt text explaining how to use the tools together,
    or None if no workflow guidance is needed (e.g., single tool).
    """
    tool_set = set(tools)

    # Discovery + Details + Execute workflow
    if {"list_functions", "get_function_details", "execute"}.issubset(tool_set):
        return """To use these tools effectively:
1. Start with list_functions to see all available SDK functions organized by namespace
2. Call get_function_details for specific functions you want to use to see their parameters and types
3. Finally, use execute to run your TypeScript code that calls those functions

This discovery -> details -> execute workflow helps you write correct code on the first try."""

    # Search + Details + Execute workflow
    if {"search_functions", "get_function_details", "execute"}.issubset(tool_set):
        return """To use these tools effectively:
1. Use search_functions to find relevant functions by keyword (searches names and descriptions)
2. Call get_function_details for the functions you want to use to see their parameters and types
3. Finally, use execute to run your TypeScript code that calls those functions

This search -> details -> execute workflow helps you quickly find and use the right functions."""

    # Exploration workflow (bash + typescript)
    if {"execute_bash", "execute_typescript"}.issubset(tool_set):
        return """To use these tools effectively:
1. Use execute_bash to explore the SDK filesystem:
   - `cat README.md` shows all available functions
   - `cat {Namespace}/{functionName}.d.ts` shows detailed type information
2. Then use execute_typescript to run your code with those functions

This exploration -> execution workflow is useful when you need to understand the SDK structure."""

    # Filesystem mode
    if {"read_file", "list_directory"} == tool_set:
        return """To use these tools effectively:
1. Use list_directory to explore the directory structure
2. Use read_file to examine specific files

These tools provide read-only filesystem access to SDK type definitions."""

    # No specific workflow needed
    return None


# Pre-defined workflows for common modes
WORKFLOW_PROMPTS = {
    "list_get_execute": get_workflow_prompt(
        ["list_functions", "get_function_details", "execute"]
    ),
    "search_get_execute": get_workflow_prompt(
        ["search_functions", "get_function_details", "execute"]
    ),
    "bash_typescript": get_workflow_prompt(["execute_bash", "execute_typescript"]),
    "fs": get_workflow_prompt(["read_file", "list_directory"]),
}
