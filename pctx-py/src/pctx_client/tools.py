"""Tool configuration for pctx client.

Provides flexible ways to configure which tools are exposed and how they're described.
"""

from dataclasses import dataclass
from typing import Literal

# All available tool names
ToolName = Literal[
    "list_functions",
    "search_functions",
    "get_function_details",
    "execute",
    "execute_bash",
    "execute_typescript",
]


@dataclass
class ToolConfig:
    """Configuration for which tools to expose and their descriptions.

    Examples:
        Pre-bundled modes:
        >>> tools = pctx.langchain_tools("list_get_execute")
        >>> tools = pctx.langchain_tools("fs")

        Override descriptions in a mode:
        >>> tools = pctx.langchain_tools(
        ...     "list_get_execute",
        ...     descriptions={"execute": "Custom description"}
        ... )

        Full control - mix and match:
        >>> from pctx_client.tools import ToolConfig
        >>> tools = pctx.langchain_tools(
        ...     ToolConfig(
        ...         tools=["execute_bash", "list_functions", "execute"],
        ...         descriptions={"execute_bash": "Custom bash description"}
        ...     )
        ... )
    """

    tools: list[ToolName]
    """List of tool names to include"""

    descriptions: dict[ToolName, str] | None = None
    """Optional custom descriptions for tools. Only overrides specified tools."""


# Pre-defined mode configurations
def list_get_execute_mode(
    descriptions: dict[ToolName, str] | None = None,
) -> ToolConfig:
    """Standard mode: list, search (if available), get_details, execute.

    This is the typical workflow:
    1. list_functions - See all available functions
    2. search_functions - Find relevant functions (if search is available)
    3. get_function_details - Get detailed info about specific functions
    4. execute - Run TypeScript code calling those functions
    """
    # Note: search_functions is included in the list even if BM25 is not installed
    # The converter methods will check HAS_SEARCH before creating the actual tool
    tools = ["list_functions", "search_functions", "get_function_details", "execute"]
    return ToolConfig(tools=tools, descriptions=descriptions)


def fs_mode(descriptions: dict[ToolName, str] | None = None) -> ToolConfig:
    """Filesystem mode: execute_bash, execute_typescript.

    This mode presents SDK functions as an in-memory filesystem:
    1. execute_bash - Explore the filesystem (ls, cat, grep .d.ts files)
    2. execute_typescript - Run TypeScript code after discovering types
    """
    return ToolConfig(
        tools=["execute_bash", "execute_typescript"], descriptions=descriptions
    )


# Type for mode strings
ModeString = Literal["list_get_execute", "fs"]


def get_toolset_from_mode(
    mode: ModeString, descriptions: dict[ToolName, str] | None = None
) -> ToolConfig:
    """Convert a mode string to a ToolConfig configuration.

    Args:
        mode: Mode name ("list_get_execute" or "fs")
        descriptions: Optional custom descriptions to override defaults

    Returns:
        ToolConfig configuration for the specified mode

    Raises:
        ValueError: If mode is not recognized
    """
    if mode == "list_get_execute":
        return list_get_execute_mode(descriptions)
    elif mode == "fs":
        return fs_mode(descriptions)
    else:
        raise ValueError(f"Unknown mode: {mode}. Valid modes: 'list_get_execute', 'fs'")
