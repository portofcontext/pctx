import asyncio
import inspect
import textwrap
from abc import ABC, abstractmethod
from collections.abc import Awaitable, Callable
from typing import Annotated, Any, get_type_hints

from docstring_parser import Docstring
from docstring_parser import parse as parse_docstring
from jsonschema import Draft202012Validator
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    PrivateAttr,
    SkipValidation,
    TypeAdapter,
    create_model,
    model_validator,
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

    input_schema: Annotated[
        type[BaseModel] | dict[str, Any] | None, SkipValidation
    ] = Field(
        default=None,
        description="The tool input schema. Either a Pydantic BaseModel class or a JSON Schema dict.",
    )

    output_schema: Annotated[Any | None, SkipValidation] = Field(
        default=None, description="The return type schema."
    )

    _input_validator: Draft202012Validator | None = PrivateAttr(default=None)

    @model_validator(mode="after")
    def _compile_input_validator(self) -> "BaseTool":
        """
        When ``input_schema`` is a JSON Schema dict, validate it against the
        Draft 2020-12 metaschema and cache a compiled validator on the
        instance so per-call ``validate_input`` doesn't recompile.

        Raises:
            jsonschema.SchemaError: If the provided dict is not a valid
                JSON Schema.
        """
        if isinstance(self.input_schema, dict):
            Draft202012Validator.check_schema(self.input_schema)
            self._input_validator = Draft202012Validator(self.input_schema)
        return self

    def validate_input(self, obj: Any):
        if self.input_schema is None:
            return
        if isinstance(self.input_schema, dict):
            assert self._input_validator is not None
            self._input_validator.validate(obj)
        else:
            self.input_schema.model_validate(obj)

    def validate_output(self, obj: Any):
        if self.output_schema is not None:
            adapter = TypeAdapter(self.output_schema)
            adapter.validate_python(obj)

    def input_json_schema(self) -> dict[str, Any] | None:
        if self.input_schema is None:
            return None
        if isinstance(self.input_schema, dict):
            return self.input_schema
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
        input_schema: type[BaseModel] | dict[str, Any] | None = None,
    ) -> "Tool | AsyncTool":
        """
        Creates a tool from a given function.

        If ``input_schema`` is provided (either a Pydantic BaseModel class or a
        JSON Schema dict), it is used as-is and signature inference is skipped.
        Otherwise the schema is inferred from the function signature.
        """

        docstring = parse_docstring(textwrap.dedent(description or func.__doc__ or ""))

        name_ = name or func.__name__

        if input_schema is None:
            in_schema = create_input_schema(f"{name_}_Input", func, docstring=docstring)
            input_schema = None if is_empty_schema(in_schema) else in_schema

        out_schema = create_output_schema(func, docstring=docstring)
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
                description=docstring.description or "",
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
                description=docstring.description or "",
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
    model_name: str, func: Callable, docstring: Docstring | None = None
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

    # Build a lookup map for parameter descriptions from docstring
    param_descriptions: dict[str, str] = {}
    if docstring and docstring.params:
        for param_doc in docstring.params:
            if param_doc.arg_name and param_doc.description:
                param_descriptions[param_doc.arg_name] = param_doc.description

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

        # Get description from docstring if available
        param_desc = param_descriptions.get(param_name)

        # Determine if the parameter is required or has a default value
        if param.default == inspect.Parameter.empty:
            # Required field - use ... as the Pydantic sentinel for required
            fields[param_name] = (annotation, Field(..., description=param_desc))
        else:
            # Optional field with default value
            fields[param_name] = (
                annotation,
                Field(default=param.default, description=param_desc),
            )

    return create_model(model_name, __config__=_MODEL_CONFIG, **fields)


def create_output_schema(func: Callable, docstring: Docstring | None = None) -> Any:
    """
    Extracts the return type annotation from a function.

    Args:
        model_name: Name for the generated Pydantic model (unused, kept for compatibility)
        func: The function to extract return type from
        docstring: Optional parsed docstring containing return description

    Returns:
        The return type annotation as a type, optionally wrapped with description metadata
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

    return Annotated[
        return_annotation,
        Field(
            description=docstring.returns.description
            if docstring and docstring.returns
            else None
        ),
    ]


def is_empty_schema(schema: type[BaseModel]) -> bool:
    json_schema = schema.model_json_schema()

    return (
        json_schema.get("type") == "object"
        and len(json_schema.get("properties", {})) == 0
    )
