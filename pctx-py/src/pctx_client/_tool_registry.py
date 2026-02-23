"""
Central registry for PCTX tools.

This module provides a single source of truth for all available tools and validates
that the registry is complete (i.e., every tool in the ToolName literal is registered).

The registry pattern ensures that:
1. All tools are defined in one place
2. Adding a new tool to ToolName requires adding it here (validated at import time)
3. Framework adapters can loop over tools instead of using if-statement chains
"""

from typing import get_args

from pctx_client.tools import ToolName

# Central registry - set of all valid tool names
TOOL_REGISTRY: set[ToolName] = {
    "list_functions",
    "search_functions",
    "get_function_details",
    "execute",
    "execute_bash",
    "execute_typescript",
    "execute_python",
}


def validate_registry_completeness() -> None:
    """
    Validate that TOOL_REGISTRY contains exactly the tools defined in ToolName.

    This ensures that:
    - Every tool in the ToolName literal is in the registry (no missing tools)
    - Every tool in the registry is in the ToolName literal (no typos/extras)

    Raises:
        ValueError: If there are missing or extra tools in the registry
    """
    all_tool_names = set(get_args(ToolName))
    registered_names = TOOL_REGISTRY

    missing = all_tool_names - registered_names
    if missing:
        raise ValueError(
            f"Missing registry entries for tools: {missing}. "
            f"Add these tools to TOOL_REGISTRY in _tool_registry.py"
        )

    extra = registered_names - all_tool_names
    if extra:
        raise ValueError(
            f"Extra registry entries not in ToolName: {extra}. "
            f"Remove these from TOOL_REGISTRY or add them to ToolName in tools.py"
        )


# Validate at module import time - fail fast if registry is incomplete
validate_registry_completeness()
