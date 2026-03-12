"""Tool descriptions loaded from versioned data files.

Provides `get_description` to retrieve tool description strings by name, disclosure style, and version.
"""

from importlib.resources import files

from pctx_client.models import ToolDisclosure, ToolDisclosureName, ToolName


def get_workflow_description(
    disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
    version: int = 1,
) -> str:
    disclosure = ToolDisclosure(disclosure)

    common = (
        files("pctx_client.descriptions.data.workflows") / "common.txt"
    ).read_text(encoding="utf-8")
    specific = (
        files("pctx_client.descriptions.data.workflows")
        / disclosure.value
        / f"v{version}.txt"
    ).read_text(encoding="utf-8")

    return f"{common}\n{specific}"


def get_tool_description(
    tool_name: ToolName,
    disclosure: ToolDisclosure | ToolDisclosureName = ToolDisclosure.CATALOG,
    version: int = 1,
    overrides: dict[ToolName, str] = None,
) -> str:
    """Load a tool description string from the data directory.

    Args:
        tool: The tool name.
        version: The version.

    Returns:
        The description text.
    """

    disclosure = ToolDisclosure(disclosure)

    if overrides is not None and tool_name in overrides:
        return overrides[tool_name]

    if tool_name == "execute_typescript":
        tool_name = f"{tool_name}_{disclosure.value}"

    return (
        files("pctx_client.descriptions.data.tools") / tool_name / f"v{version}.txt"
    ).read_text(encoding="utf-8")


__all__ = ["ToolName", "get_tool_description", "get_workflow_description"]
