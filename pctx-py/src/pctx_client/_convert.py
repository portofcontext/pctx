from collections.abc import Callable
from typing import Any, overload

from pctx_client._tool import AsyncTool, Tool


@overload
def tool(
    name_or_callable: str,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    infer_return_type: bool = False,
) -> Callable[[Callable], Tool | AsyncTool]: ...
@overload
def tool(
    name_or_callable: Callable,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    infer_return_type: bool = False,
) -> Tool | AsyncTool: ...
@overload
def tool(
    name_or_callable: None = None,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    infer_return_type: bool = False,
) -> Callable[[Callable], Tool | AsyncTool]: ...


def tool(
    name_or_callable: str | Callable | None = None,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    infer_return_type: bool = False,
) -> Tool | AsyncTool | Callable[[Callable], Tool | AsyncTool]:
    """
    Decorator that converts a function into a Tool or AsyncTool instance.

    Can be used with or without parameters:
    - @tool - Uses function name as tool name
    - @tool("custom_name") - Uses custom name for the tool
    - @tool(namespace="custom", description="...") - With additional options
    - @tool(infer_return_type=True) - Infer return type using Jedi

    Args:
        name_or_callable: Either a custom tool name (str) or the function to wrap (Callable)
        namespace: The namespace the tool belongs to (default: "tools")
        description: Optional description override (default: uses function docstring)
        infer_return_type: If True and function has no return annotation,
                          attempt to infer it using Jedi static analysis.
                          Requires: pip install pctx-client[jedi]

    Returns:
        Either a Tool/AsyncTool instance or a decorator function that creates one

    Examples:
        >>> @tool
        ... def my_function(x: int) -> int:
        ...     '''Adds one to x'''
        ...     return x + 1

        >>> @tool("custom_name", namespace="math")
        ... def add_two(x: int) -> int:
        ...     return x + 2

        >>> @tool(infer_return_type=True)
        ... def inferred(x: str):
        ...     return {"result": x}
    """

    def _crate_tool_factory(tool_name: str | None) -> Callable[[Callable], Tool | AsyncTool]:
        """
        Creates a decorator which takes the callable & returns the tool

        Args:
            tool_name: the unique name of the tool, or None to use function name

        Returns:
            A function that takes a callable & returns a base tool
        """

        def _tool_factory(fn: Callable) -> Tool | AsyncTool:
            tool_desc = description
            # Use provided name or fall back to function name
            final_name = tool_name if tool_name is not None else fn.__name__

            return Tool.from_func(
                func=fn,
                name=final_name,
                namespace=namespace,
                description=tool_desc,
                infer_return_type=infer_return_type,
            )

        return _tool_factory

    if len(args) != 0:
        raise ValueError("Too many arguments for @tool decorator")

    if name_or_callable is None:
        # decorator used with keyword-only params
        # @tool(infer_return_type=True)
        # def some_tool():
        #     pass
        return _crate_tool_factory(None)
    elif isinstance(name_or_callable, str):
        # decorator used with name param
        # @tool("other_tool")
        # def some_tool():
        #     pass
        return _crate_tool_factory(name_or_callable)
    elif callable(name_or_callable) and hasattr(name_or_callable, "__name__"):
        # decorator used without params
        # @tool
        # def some_tool():
        #     pass
        return _crate_tool_factory(name_or_callable.__name__)(name_or_callable)
    else:
        raise ValueError(
            f"The first arg of the tool decorator must be a string, callable with a __name__ attribute, or None. Got {type(name_or_callable)}"
        )
