from collections.abc import Callable
from typing import Any, overload

from pydantic import BaseModel

from pctx_client._tool import AsyncTool, Tool


@overload
def tool(
    fn: Callable,
    *,
    name: str | None = None,
    namespace: str = "tools",
    description: str | None = None,
    input_schema: type[BaseModel] | dict[str, Any] | None = None,
    output_schema: Any | None = None,
) -> Tool | AsyncTool: ...
@overload
def tool(
    fn: None = None,
    *,
    name: str | None = None,
    namespace: str = "tools",
    description: str | None = None,
    input_schema: type[BaseModel] | dict[str, Any] | None = None,
    output_schema: Any | None = None,
) -> Callable[[Callable], Tool | AsyncTool]: ...


def tool(
    fn: Callable | None = None,
    *,
    name: str | None = None,
    namespace: str = "tools",
    description: str | None = None,
    input_schema: type[BaseModel] | dict[str, Any] | None = None,
    output_schema: Any | None = None,
) -> Tool | AsyncTool | Callable[[Callable], Tool | AsyncTool]:
    """
    Decorator that converts a function into a Tool or AsyncTool instance.

    Can be used with or without parameters:
    - @tool - Uses function name as tool name
    - @tool(name="custom_name") - Uses custom name for the tool
    - @tool(namespace="custom", description="...") - With additional options
    - @tool(input_schema=MyModel) or @tool(input_schema={...}) - Override
      signature inference with an explicit Pydantic model or JSON Schema dict
    - @tool(output_schema={...}) or @tool(output_schema=MyModel) - Override
      return-annotation inference with an explicit JSON Schema dict or
      Python type / typing construct

    Args:
        fn: The function to wrap. Only set when used as a bare ``@tool``
            decorator; in the parameterized form ``@tool(...)`` it is None
            and the function is supplied on the second call.
        name: Optional custom tool name (default: function's ``__name__``).
        namespace: The namespace the tool belongs to (default: "tools").
        description: Optional description override (default: uses function docstring).
        input_schema: Optional explicit input schema. When provided, signature
            inference is skipped and this schema is used directly.
        output_schema: Optional explicit output schema. When provided, return-
            annotation inference is skipped and this schema is used directly.

    Returns:
        Either a Tool/AsyncTool instance (bare form) or a decorator function
        that creates one (parameterized form).

    Examples:
        >>> @tool
        ... def my_function(x: int) -> int:
        ...     '''Adds one to x'''
        ...     return x + 1

        >>> @tool(name="custom_name", namespace="math")
        ... def add_two(x: int) -> int:
        ...     return x + 2

        >>> @tool(input_schema={"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]})
        ... def from_jsonschema(**kwargs) -> int:
        ...     return kwargs["x"] + 1
    """

    def _factory(f: Callable) -> Tool | AsyncTool:
        return Tool.from_func(
            func=f,
            name=name,
            namespace=namespace,
            description=description,
            input_schema=input_schema,
            output_schema=output_schema,
        )

    if fn is None:
        # Parameterized form: @tool(name=..., input_schema=...) — return the
        # factory so Python applies it to the decorated function on the next call.
        return _factory

    if not callable(fn):
        raise TypeError(
            f"@tool's positional argument must be the decorated callable, got {type(fn).__name__}"
        )

    # Bare form: @tool — fn is the decorated callable, build the Tool now.
    return _factory(fn)
