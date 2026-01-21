import asyncio
import inspect
import textwrap
from abc import ABC, abstractmethod
from collections.abc import Awaitable, Callable
from typing import Annotated, Any, get_type_hints

from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    SkipValidation,
    TypeAdapter,
    create_model,
)


class BaseTool(BaseModel):
    name: str
    """
    Unique name of tool that clearly specifies it's purpose
    """

    namespace: str
    """
    Namespace the tool belongs in
    """

    description: str = ""
    """
    Longer-form text which instructs the model how/why/when to use the tool.
    """

    input_schema: Annotated[type[BaseModel] | None, SkipValidation] = Field(
        default=None, description="The tool schema."
    )

    output_schema: Annotated[Any | None, SkipValidation] = Field(
        default=None, description="The return type schema."
    )

    def validate_input(self, obj: Any):
        if self.input_schema is not None:
            self.input_schema.model_validate(obj)

    def validate_output(self, obj: Any):
        if self.output_schema is not None:
            adapter = TypeAdapter(self.output_schema)
            adapter.validate_python(obj)

    def input_json_schema(self) -> dict[str, Any] | None:
        if self.input_schema is None:
            return None

        return self.input_schema.model_json_schema()

    def output_json_schema(self) -> dict[str, Any] | None:
        if self.output_schema is None:
            return None

        adapter = TypeAdapter(self.output_schema)
        return adapter.json_schema()

    @classmethod
    def from_func(
        cls,
        func: Callable | Callable[..., Awaitable[Any]],
        name: str | None = None,
        namespace: str = "tools",
        description: str | None = None,
        infer_return_type: bool = False,
    ) -> "Tool | AsyncTool":
        """
        Creates a tool from a given function.

        Args:
            func: The function to wrap
            name: Optional custom name (defaults to function name)
            namespace: Namespace the tool belongs to
            description: Optional description (defaults to docstring)
            infer_return_type: If True, attempt to infer return type using Jedi
                              when no annotation is present
        """

        if description is None:
            # use function doc string & remove indents
            _desc = textwrap.dedent(func.__doc__ or "")
        else:
            _desc = description

        name_ = name or func.__name__

        in_schema = create_input_schema(f"{name_}_Input", func)
        out_schema = create_output_schema(func, infer_with_jedi=infer_return_type)

        input_schema = None if is_empty_schema(in_schema) else in_schema
        output_schema = out_schema

        # Create concrete tool classes based on sync vs async
        if asyncio.iscoroutinefunction(func):
            # Asynchronous tool
            class _CoroutineTool(AsyncTool):
                """Concrete asynchronous tool wrapping a coroutine"""

                _coroutine: Callable[..., Awaitable[Any]] = staticmethod(func)

                async def _ainvoke(self, **kwargs: Any) -> Any:
                    return await self._coroutine(**kwargs)

            return _CoroutineTool(
                name=name_,
                namespace=namespace,
                description=_desc,
                input_schema=input_schema,
                output_schema=output_schema,
            )
        else:
            # Synchronous tool
            class _FunctionTool(Tool):
                """Synchronous tool wrapping decorated function"""

                _func: Callable = staticmethod(func)

                def _invoke(self, **kwargs: Any) -> Any:
                    return self._func(**kwargs)

            return _FunctionTool(
                name=name_,
                namespace=namespace,
                description=_desc,
                input_schema=input_schema,
                output_schema=output_schema,
            )


class Tool(BaseTool, ABC):
    """
    Synchronous tool base class
    """

    @abstractmethod
    def _invoke(self, **kwargs) -> Any:
        """
        Sync implementation of the tool.

        Subclasses must implement this method for synchronous execution.

        Args:
            *args: Positional arguments for the tool.
            **kwargs: Keyword arguments for the tool.

        Returns:
            The result of the tool execution.
        """

    def invoke(self, **kwargs: Any) -> Any:
        """
        Calls the synchronous function with the provided arguments.

        Args:
            **kwargs: Arguments to pass to the function

        Returns:
            The result of the function call

        Raises:
            ValueError: If no synchronous function is available
        """

        self.validate_input(kwargs)

        output = self._invoke(**kwargs)

        self.validate_output(output)

        return output


class AsyncTool(BaseTool, ABC):
    """
    Asynchronous tool base class
    """

    @abstractmethod
    async def _ainvoke(self, **kwargs) -> Any:
        """
        Async implementation of the tool.

        Subclasses must implement this method for asynchronous execution.

        Args:
            *args: Positional arguments for the tool.
            **kwargs: Keyword arguments for the tool.

        Returns:
            The result of the tool execution.
        """

    async def ainvoke(self, **kwargs: Any) -> Any:
        """
        Calls the asynchronous function with the provided arguments.

        Args:
            **kwargs: Arguments to pass to the function

        Returns:
            The result of the function call

        Raises:
            ValueError: If no synchronous function is available
        """

        self.validate_input(kwargs)

        output = await self._ainvoke(**kwargs)

        self.validate_output(output)

        return output


_MODEL_CONFIG: ConfigDict = {"extra": "forbid", "arbitrary_types_allowed": True}


def create_input_schema(
    model_name: str,
    func: Callable,
) -> type[BaseModel]:
    """
    Creates pydantic model from function signature.

    Args:
        model_name: Name for the generated Pydantic model
        func: The function to extract parameters from

    Returns:
        A dynamically created Pydantic BaseModel class
    """
    sig = inspect.signature(func)

    # Build field definitions for create_model
    fields: dict[str, Any] = {}

    for param_name, param in sig.parameters.items():
        # Skip *args and **kwargs
        if param.kind in (
            inspect.Parameter.VAR_POSITIONAL,
            inspect.Parameter.VAR_KEYWORD,
        ):
            continue

        # Get type annotation (default to Any if not specified)
        annotation = (
            param.annotation if param.annotation != inspect.Parameter.empty else Any
        )

        # Determine if the parameter is required or has a default value
        if param.default == inspect.Parameter.empty:
            # Required field - use ... as the Pydantic sentinel for required
            fields[param_name] = (annotation, ...)
        else:
            # Optional field with default value
            fields[param_name] = (annotation, param.default)

    return create_model(model_name, __config__=_MODEL_CONFIG, **fields)


def create_output_schema(
    func: Callable,
    infer_with_jedi: bool = False,
) -> Any:
    """
    Extracts the return type annotation from a function.

    Args:
        func: The function to extract return type from
        infer_with_jedi: If True and no annotation exists, attempt to infer
                        the return type using Jedi static analysis (requires
                        jedi optional dependency)

    Returns:
        The return type annotation as a type
    """
    # Use get_type_hints to resolve string annotations to actual types
    # This handles cases where the calling code uses "from __future__ import annotations"
    try:
        type_hints = get_type_hints(func)
        return_annotation = type_hints.get("return", Any)
    except Exception:
        # Fallback to inspect if get_type_hints fails
        sig = inspect.signature(func)
        return_annotation = (
            sig.return_annotation if sig.return_annotation is not sig.empty else Any
        )

    # If we got a real type, use it
    if return_annotation is not Any:
        return return_annotation

    # If Jedi inference is enabled and no annotation found, try Jedi
    if infer_with_jedi:
        from pctx_client._jedi_infer import HAS_JEDI, infer_return_type

        if not HAS_JEDI:
            import warnings

            warnings.warn(
                "Jedi inference requested but jedi is not installed. "
                "Install with: pip install pctx-client[jedi]",
                UserWarning,
                stacklevel=3,
            )
            return Any

        return infer_return_type(func)

    return return_annotation


def is_empty_schema(schema: type[BaseModel]) -> bool:
    json_schema = schema.model_json_schema()

    return (
        json_schema.get("type") == "object"
        and len(json_schema.get("properties", {})) == 0
    )
