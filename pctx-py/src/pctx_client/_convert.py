from collections.abc import Callable
from typing import Any, overload

from pydantic import BaseModel

from pctx_client._tool import AsyncTool, Tool


@overload
def tool(
    name_or_callable: str,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    input_schema: type[BaseModel] | dict[str, Any] | None = None,
) -> Callable[[Callable], Tool | AsyncTool]: ...
@overload
def tool(
    name_or_callable: Callable,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    input_schema: type[BaseModel] | dict[str, Any] | None = None,
) -> Tool | AsyncTool: ...


def tool(
    name_or_callable: str | Callable,
    *args: Any,
    namespace: str = "tools",
    description: str | None = None,
    input_schema: type[BaseModel] | dict[str, Any] | None = None,
) -> Tool | AsyncTool | Callable[[Callable], Tool | AsyncTool]:
    """
    Decorator that converts a function into a Tool or AsyncTool instance.

    Can be used with or without parameters:
    - @tool - Uses function name as tool name
    - @tool("custom_name") - Uses custom name for the tool
    - @tool(namespace="custom", description="...") - With additional options
    - @tool(input_schema=MyModel) or @tool(input_schema={...}) - Override
      signature inference with an explicit Pydantic model or JSON Schema dict

    Args:
        name_or_callable: Either a custom tool name (str) or the function to wrap (Callable)
        namespace: The namespace the tool belongs to (default: "tools")
        description: Optional description override (default: uses function docstring)
        input_schema: Optional explicit input schema. When provided, signature
            inference is skipped and this schema is used directly.

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

        >>> @tool(input_schema={"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]})
        ... def from_jsonschema(**kwargs) -> int:
        ...     return kwargs["x"] + 1
    """

    def _crate_tool_factory(tool_name: str) -> Callable[[Callable], Tool | AsyncTool]:
        """
        Creates a decorator which takes the callable & returns the tool

        Args:
            tool_name: the unique name of the tool

        Returns:
            A function that takes a callable & returns a base tool
        """

        def _tool_factory(fn: Callable) -> Tool | AsyncTool:
            return Tool.from_func(
                func=fn,
                name=tool_name,
                namespace=namespace,
                description=description,
                input_schema=input_schema,
            )

        return _tool_factory

    if len(args) != 0:
        raise ValueError("Too many arguments for @tool decorator")

    if isinstance(name_or_callable, str):
        # decorator used with params
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
            f"The first arg of the tool decorator must be a string or a callable with a __name__ attribute. Got {type(name_or_callable)}"
        )
