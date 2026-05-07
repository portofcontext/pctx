"""Tests for the @tool decorator in pctx_py.tools.convert"""

from __future__ import annotations

import jsonschema
import pytest
from pydantic import BaseModel, ValidationError

from pctx_client import Tool, tool
from pctx_client._tool import AsyncTool

# ============================================================================
# SECTION 1: REGISTRATION TESTS
# Tests for Tool attributes: name, description, args_schema, func, coroutine
# ============================================================================


def test_registration_basic_sync_function() -> None:
    """Test basic tool registration with sync function"""

    @tool
    def simple_function() -> str:
        """A simple test function"""
        return "result"

    assert isinstance(simple_function, Tool)
    assert simple_function.name == "simple_function"
    assert simple_function.description == "A simple test function"
    assert simple_function.input_json_schema() is None
    assert simple_function.output_json_schema() == {"type": "string"}


def test_registration_basic_async_function() -> None:
    """Test basic tool registration with async function"""

    @tool
    async def async_function() -> str:
        """An async test function"""
        return "async result"

    assert isinstance(async_function, AsyncTool)
    assert async_function.name == "async_function"
    assert async_function.description == "An async test function"
    assert async_function.input_json_schema() is None
    assert async_function.output_json_schema() == {"type": "string"}


def test_registration_custom_name() -> None:
    """Test tool registration with custom name"""

    @tool("custom_name")
    def my_function() -> str:
        """Function with custom name"""
        return "result"

    assert my_function.name == "custom_name"
    assert my_function.description == "Function with custom name"


def test_registration_custom_description() -> None:
    """Test tool registration with custom description"""

    @tool("tool_name", description="Custom description here")
    def my_function() -> str:
        """Original docstring"""
        return "result"

    assert my_function.name == "tool_name"
    assert my_function.description == "Custom description here"


def test_registration_with_parameters() -> None:
    """Test tool registration with function parameters in args_schema"""

    @tool
    def add_numbers(a: int, b: int, c: str = "default") -> str:
        """Adds two numbers"""
        return str(a + b)

    # Check args_schema includes parameters
    assert (
        add_numbers.input_schema is not None
        and add_numbers.input_schema.model_json_schema()
        == {
            "title": "add_numbers_Input",
            "type": "object",
            "required": ["a", "b"],
            "properties": {
                "a": {"title": "A", "type": "integer"},
                "b": {"title": "B", "type": "integer"},
                "c": {"title": "C", "type": "string", "default": "default"},
            },
            "additionalProperties": False,
        }
    )


def test_registration_docstring_becomes_description() -> None:
    """Test that function docstring becomes tool description"""

    @tool
    def documented_function() -> str:
        """This is a detailed description
        of what the function does."""
        return "result"

    assert "This is a detailed description" in documented_function.description
    assert "of what the function does." in documented_function.description


def test_registration_indented_docstring_dedented() -> None:
    """Test that indented docstrings are properly dedented"""

    @tool
    def indented_doc() -> str:
        """
        First line
            Indented line
        Last line
        """
        return "result"

    assert indented_doc.description == "First line\nIndented line\nLast line"


def test_registration_no_docstring() -> None:
    """Test function without docstring has empty description"""

    @tool
    def no_doc() -> str:
        return "result"

    assert no_doc.description == ""


def test_registration_custom_description_overrides_docstring() -> None:
    """Test that custom description overrides docstring"""

    @tool("func", description="Custom")
    def with_docstring() -> str:
        """Original docstring"""
        return "result"

    assert with_docstring.description == "Custom"


def test_registration_multipletools_independent() -> None:
    """Test that multiple decorated functions are independent"""

    @tool
    def tool_one() -> str:
        """First tool"""
        return "one"

    @tool
    def tool_two() -> str:
        """Second tool"""
        return "two"

    assert isinstance(tool_one, Tool)
    assert isinstance(tool_two, Tool)
    assert tool_one.name == "tool_one"
    assert tool_two.name == "tool_two"
    assert tool_one.description == "First tool"
    assert tool_two.description == "Second tool"


def test_registration_error_too_many_arguments() -> None:
    """Test that providing too many arguments raises ValueError"""

    with pytest.raises(ValueError, match="Too many arguments"):

        @tool("name", "extra_arg")
        def bad_function() -> str:
            return "result"


def test_registration_error_invalid_first_argument() -> None:
    """Test that invalid first argument raises ValueError"""

    with pytest.raises(
        ValueError, match="must be a string or a callable with a __name__"
    ):
        tool(123)  # type: ignore


def test_registration_error_callable_without_name() -> None:
    """Test that callable without __name__ raises ValueError"""

    class CallableWithoutName:
        def __call__(self) -> str:
            return "result"

    obj = CallableWithoutName()
    with pytest.raises(
        ValueError, match="must be a string or a callable with a __name__"
    ):
        tool(obj)  # type: ignore


# ============================================================================
# SECTION 2: CALLING FUNCTIONS
# Tests for actually calling the registered sync and async functions
# ============================================================================


def test_calling_sync_function_no_parameters() -> None:
    """Test calling sync function with no parameters"""

    @tool
    def synctool() -> str:
        """Sync function"""
        return "sync result"

    assert isinstance(synctool, Tool)
    result = synctool.invoke()
    assert result == "sync result"


def test_calling_sync_function_with_positional_args() -> None:
    """Test calling sync function with positional arguments"""

    @tool
    def add_numbers(a: int, b: int) -> str:
        """Adds two numbers"""
        return str(a + b)

    assert isinstance(add_numbers, Tool)
    result = add_numbers.invoke(a=5, b=3)
    assert result == "8"


def test_calling_sync_function_with_kwargs() -> None:
    """Test calling sync function with keyword arguments"""

    @tool
    def greet(name: str, greeting: str = "Hello") -> str:
        """Greets a person"""
        return f"{greeting}, {name}!"

    assert isinstance(greet, Tool)

    # Test with default
    result1 = greet.invoke(name="Alice")
    assert result1 == "Hello, Alice!"

    # Test with custom kwarg
    result2 = greet.invoke(name="Bob", greeting="Hi")
    assert result2 == "Hi, Bob!"


def test_calling_sync_function_with_mixed_args() -> None:
    """Test calling sync function with both positional and keyword arguments"""

    @tool
    def process(x: int, y: int, multiplier: int = 2) -> str:
        """Process two numbers"""
        return str((x + y) * multiplier)

    assert isinstance(process, Tool)

    # Test with default multiplier
    result1 = process.invoke(x=3, y=4)
    assert result1 == "14"  # (3 + 4) * 2

    # Test with custom multiplier
    result2 = process.invoke(x=3, y=4, multiplier=3)
    assert result2 == "21"  # (3 + 4) * 3


@pytest.mark.asyncio
async def test_calling_async_function_no_parameters() -> None:
    """Test calling async function with no parameters"""

    @tool
    async def asynctool() -> str:
        """Async function"""
        return "async result"

    assert isinstance(asynctool, AsyncTool)
    result = await asynctool.ainvoke()
    assert result == "async result"


@pytest.mark.asyncio
async def test_calling_async_function_with_parameters() -> None:
    """Test calling async function with parameters"""

    @tool
    async def fetch_data(url: str, timeout: int = 30) -> str:
        """Fetches data from URL"""
        return f"Data from {url} with timeout {timeout}"

    assert isinstance(fetch_data, AsyncTool)

    # Test with custom timeout
    result = await fetch_data.ainvoke(url="https://example.com", timeout=60)
    assert result == "Data from https://example.com with timeout 60"


@pytest.mark.asyncio
async def test_calling_async_function_with_defaults() -> None:
    """Test calling async function using default parameters"""

    @tool
    async def fetch_data(url: str, timeout: int = 30, retries: int = 3) -> str:
        """Fetches data from URL"""
        return f"URL: {url}, timeout: {timeout}, retries: {retries}"

    assert isinstance(fetch_data, AsyncTool)

    # Test with all defaults
    result1 = await fetch_data.ainvoke(url="https://test.com")
    assert result1 == "URL: https://test.com, timeout: 30, retries: 3"

    # Test with partial kwargs
    result2 = await fetch_data.ainvoke(url="https://test.com", retries=5)
    assert result2 == "URL: https://test.com, timeout: 30, retries: 5"


def test_calling_sync_function_multiple_calls() -> None:
    """Test that sync function can be called multiple times"""

    call_count = 0

    @tool
    def counter() -> str:
        nonlocal call_count
        call_count += 1
        return f"Call {call_count}"

    assert isinstance(counter, Tool)

    assert counter.invoke() == "Call 1"
    assert counter.invoke() == "Call 2"
    assert counter.invoke() == "Call 3"


@pytest.mark.asyncio
async def test_calling_async_function_multiple_calls() -> None:
    """Test that async function can be called multiple times"""

    call_count = 0

    @tool
    async def async_counter() -> str:
        nonlocal call_count
        call_count += 1
        return f"Async call {call_count}"

    assert isinstance(async_counter, AsyncTool)

    assert await async_counter.ainvoke() == "Async call 1"
    assert await async_counter.ainvoke() == "Async call 2"
    assert await async_counter.ainvoke() == "Async call 3"


# ============================================================================
# SECTION 3: VALIDATION TESTS
# Tests for input validation with invoke/ainvoke methods
# ============================================================================


def test_validation_missing_required_parameter() -> None:
    """Test that missing required parameters raise ValidationError"""

    @tool
    def add_numbers(a: int, b: int) -> str:
        """Adds two numbers"""
        return str(a + b)

    assert isinstance(add_numbers, Tool)

    # Missing parameter 'b'
    with pytest.raises(ValidationError) as exc_info:
        add_numbers.invoke(a=5)

    assert "b" in str(exc_info.value)


def test_validation_wrong_type_parameter() -> None:
    """Test that wrong type parameters raise ValidationError"""

    @tool
    def add_numbers(a: int, b: int) -> str:
        """Adds two numbers"""
        return str(a + b)

    assert isinstance(add_numbers, Tool)

    # Wrong type for parameter 'b'
    with pytest.raises(ValidationError) as exc_info:
        add_numbers.invoke(a=5, b="not_an_int")

    assert (
        "b" in str(exc_info.value).lower()
        or "validation" in str(exc_info.value).lower()
    )


def test_validation_extra_parameter() -> None:
    """Test that extra parameters raise ValidationError"""

    @tool
    def add_numbers(a: int, b: int) -> str:
        """Adds two numbers"""
        return str(a + b)

    assert isinstance(add_numbers, Tool)

    # Extra parameter 'c' not defined in schema
    with pytest.raises(ValidationError) as exc_info:
        add_numbers.invoke(a=5, b=3, c=10)

    assert "extra" in str(exc_info.value).lower() or "c" in str(exc_info.value).lower()


def test_validation_valid_input_with_defaults() -> None:
    """Test that valid input with defaults passes validation"""

    @tool
    def greet(name: str, greeting: str = "Hello") -> str:
        """Greets a person"""
        return f"{greeting}, {name}!"

    assert isinstance(greet, Tool)

    # Should not raise any validation error
    result = greet.invoke(name="Alice")
    assert result == "Hello, Alice!"


def test_validation_valid_input_all_parameters() -> None:
    """Test that valid input with all parameters passes validation"""

    @tool
    def process(x: int, y: int, multiplier: int = 2) -> str:
        """Process two numbers"""
        return str((x + y) * multiplier)

    assert isinstance(process, Tool)

    # Should not raise any validation error
    result = process.invoke(x=3, y=4, multiplier=5)
    assert result == "35"


@pytest.mark.asyncio
async def test_validation_async_missing_required_parameter() -> None:
    """Test that async functions validate missing required parameters"""

    @tool
    async def fetch_data(url: str, timeout: int = 30) -> str:
        """Fetches data from URL"""
        return f"Data from {url} with timeout {timeout}"

    assert isinstance(fetch_data, AsyncTool)

    # Missing required parameter 'url'
    with pytest.raises(ValidationError) as exc_info:
        await fetch_data.ainvoke(timeout=60)

    assert "url" in str(exc_info.value)


@pytest.mark.asyncio
async def test_validation_async_wrong_type_parameter() -> None:
    """Test that async functions validate parameter types"""

    @tool
    async def fetch_data(url: str, timeout: int = 30) -> str:
        """Fetches data from URL"""
        return f"Data from {url} with timeout {timeout}"

    assert isinstance(fetch_data, AsyncTool)

    # Wrong type for 'timeout' parameter
    with pytest.raises(ValidationError) as exc_info:
        await fetch_data.ainvoke(url="https://example.com", timeout="not_an_int")

    assert (
        "timeout" in str(exc_info.value).lower()
        or "validation" in str(exc_info.value).lower()
    )


@pytest.mark.asyncio
async def test_validation_async_valid_input() -> None:
    """Test that async functions accept valid input"""

    @tool
    async def fetch_data(url: str, timeout: int = 30) -> str:
        """Fetches data from URL"""
        return f"Data from {url} with timeout {timeout}"

    assert isinstance(fetch_data, AsyncTool)

    # Should not raise any validation error
    result = await fetch_data.ainvoke(url="https://example.com", timeout=60)
    assert result == "Data from https://example.com with timeout 60"


# ============================================================================
# SECTION 4: DOCSTRING PARSING TESTS
# Tests for parsing various docstring formats (Google, NumPy, REST, Epydoc)
# ============================================================================


# Docstring formats for parametrized testing
GOOGLE_DOCSTRING = """Calculate the Euclidean distance between two points.

This function computes the distance between two points in 2D space
using the Euclidean distance formula.

Args:
    x1: The x-coordinate of the first point
    y1: The y-coordinate of the first point
    x2: The x-coordinate of the second point (defaults to origin)
    y2: The y-coordinate of the second point (defaults to origin)

Returns:
    The Euclidean distance between the two points as a floating point number

Raises:
    ValueError: If coordinates are invalid
"""

NUMPY_DOCSTRING = """Calculate the Euclidean distance between two points.

This function computes the distance between two points in 2D space
using the Euclidean distance formula.

Parameters
----------
x1 : float
    The x-coordinate of the first point
y1 : float
    The y-coordinate of the first point
x2 : float, optional
    The x-coordinate of the second point (defaults to origin)
y2 : float, optional
    The y-coordinate of the second point (defaults to origin)

Returns
-------
float
    The Euclidean distance between the two points as a floating point number

Raises
------
ValueError
    If coordinates are invalid
"""

REST_DOCSTRING = """Calculate the Euclidean distance between two points.

This function computes the distance between two points in 2D space
using the Euclidean distance formula.

:param x1: The x-coordinate of the first point
:type x1: float
:param y1: The y-coordinate of the first point
:type y1: float
:param x2: The x-coordinate of the second point (defaults to origin)
:type x2: float
:param y2: The y-coordinate of the second point (defaults to origin)
:type y2: float
:returns: The Euclidean distance between the two points as a floating point number
:rtype: float
:raises ValueError: If coordinates are invalid
"""

EPYDOC_DOCSTRING = """Calculate the Euclidean distance between two points.

This function computes the distance between two points in 2D space
using the Euclidean distance formula.

@param x1: The x-coordinate of the first point
@type x1: float
@param y1: The y-coordinate of the first point
@type y1: float
@param x2: The x-coordinate of the second point (defaults to origin)
@type x2: float
@param y2: The y-coordinate of the second point (defaults to origin)
@type y2: float
@return: The Euclidean distance between the two points as a floating point number
@rtype: float
@raise ValueError: If coordinates are invalid
"""


@pytest.mark.parametrize(
    "docstring_format,docstring_text",
    [
        ("google", GOOGLE_DOCSTRING),
        ("numpy", NUMPY_DOCSTRING),
        ("rest", REST_DOCSTRING),
        ("epydoc", EPYDOC_DOCSTRING),
    ],
)
def test_docstring_parsing_formats(docstring_format: str, docstring_text: str) -> None:
    """Test that various docstring formats are parsed correctly into tool metadata"""

    def calculate_distance(
        x1: float, y1: float, x2: float = 0.0, y2: float = 0.0
    ) -> float:
        import math

        return math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2)

    calculate_distance.__doc__ = docstring_text

    # Apply tool decorator
    decorated_tool = tool(calculate_distance)

    assert isinstance(decorated_tool, Tool)
    assert decorated_tool.name == "calculate_distance"

    # Test that description contains the short & long description
    assert (
        "Calculate the Euclidean distance between two points"
        in decorated_tool.description
    )
    assert "2D space" in decorated_tool.description
    assert "Euclidean distance formula" in decorated_tool.description

    # Test that input schema includes parameter descriptions
    input_schema = decorated_tool.input_json_schema()
    assert input_schema is not None

    properties = input_schema["properties"]
    assert "x1" in properties
    assert "y1" in properties
    assert "x2" in properties
    assert "y2" in properties

    # Verify parameter descriptions are extracted from docstring
    assert "x-coordinate" in properties["x1"]["description"]
    assert "first point" in properties["x1"]["description"]
    assert "y-coordinate" in properties["y1"]["description"]
    assert "first point" in properties["y1"]["description"]
    assert "x-coordinate" in properties["x2"]["description"]
    assert "second point" in properties["x2"]["description"]
    assert "y-coordinate" in properties["y2"]["description"]
    assert "second point" in properties["y2"]["description"]

    # Verify required and optional parameters are correct
    assert "x1" in input_schema["required"]
    assert "y1" in input_schema["required"]
    assert properties["x2"]["default"] == 0.0
    assert properties["y2"]["default"] == 0.0

    # Test that output schema includes return description
    output_schema = decorated_tool.output_json_schema()
    assert output_schema is not None
    assert output_schema["type"] == "number"
    assert "description" in output_schema
    assert "Euclidean distance" in output_schema["description"]
    assert "floating point" in output_schema["description"]


# ============================================================================
# SECTION: EXPLICIT input_schema (skips signature inference)
# ============================================================================


def test_input_schema_dict_skips_inference() -> None:
    """An explicit JSON Schema dict is used as-is and signature is not inspected."""

    schema: dict = {
        "type": "object",
        "properties": {"x": {"type": "integer"}},
        "required": ["x"],
        "additionalProperties": False,
    }

    @tool("from_dict", input_schema=schema)
    def from_dict(**kwargs) -> int:
        return kwargs["x"] + 1

    assert isinstance(from_dict, Tool)
    # Schema is the exact dict the user passed, not derived from the signature.
    assert from_dict.input_json_schema() == schema
    assert from_dict.invoke(x=4) == 5


def test_input_schema_dict_rejects_invalid_input() -> None:
    """Dict-defined input_schema validates via jsonschema and rejects bad input."""

    @tool(
        "add_one",
        input_schema={
            "type": "object",
            "properties": {"x": {"type": "integer"}},
            "required": ["x"],
        },
    )
    def add_one(**kwargs) -> int:
        return kwargs["x"] + 1

    with pytest.raises(jsonschema.ValidationError):
        add_one.invoke(x="not-an-int")

    with pytest.raises(jsonschema.ValidationError):
        add_one.invoke()  # missing required `x`


def test_input_schema_dict_malformed_raises_at_construction() -> None:
    """A malformed JSON Schema is rejected when the tool is built, not on first call."""

    with pytest.raises(jsonschema.SchemaError):

        @tool("bad", input_schema={"type": "not-a-real-type"})
        def bad(**kwargs) -> int:
            return 0


def test_input_schema_pydantic_model_skips_inference() -> None:
    """An explicit Pydantic BaseModel class is used as-is, ignoring the signature."""

    class Args(BaseModel):
        y: float

    @tool("from_model", input_schema=Args)
    def from_model(**kwargs) -> float:
        return kwargs["y"] * 2

    assert from_model.input_schema is Args
    schema = from_model.input_json_schema()
    assert schema is not None
    assert schema["properties"] == {"y": {"title": "Y", "type": "number"}}
    assert from_model.invoke(y=1.5) == 3.0


def test_input_schema_pydantic_model_rejects_invalid_input() -> None:
    """Pydantic-defined input_schema raises pydantic.ValidationError on bad input."""

    class Args(BaseModel):
        y: float

    @tool("from_model", input_schema=Args)
    def from_model(**kwargs) -> float:
        return kwargs["y"] * 2

    with pytest.raises(ValidationError):
        from_model.invoke(y="not-a-float")


def test_input_schema_explicit_overrides_signature() -> None:
    """When input_schema is provided, the function's own annotations are ignored."""

    schema: dict = {
        "type": "object",
        "properties": {"q": {"type": "string"}},
        "required": ["q"],
    }

    # Function signature says `n: int`, but we pass an unrelated schema.
    # The explicit schema wins; signature inference is skipped entirely.
    @tool("search", input_schema=schema)
    def search(n: int = 0, **kwargs) -> str:
        return kwargs.get("q", "")

    assert search.input_json_schema() == schema
    assert search.invoke(q="hello") == "hello"


async def test_input_schema_dict_async() -> None:
    """Explicit JSON Schema dict works for async tools too."""

    @tool(
        "add_one_async",
        input_schema={
            "type": "object",
            "properties": {"x": {"type": "integer"}},
            "required": ["x"],
        },
    )
    async def add_one(**kwargs) -> int:
        return kwargs["x"] + 1

    assert isinstance(add_one, AsyncTool)
    assert await add_one.ainvoke(x=4) == 5

    with pytest.raises(jsonschema.ValidationError):
        await add_one.ainvoke(x="bad")


def test_input_schema_with_custom_name_and_namespace() -> None:
    """Explicit input_schema composes with custom name + namespace."""

    schema: dict = {
        "type": "object",
        "properties": {"n": {"type": "integer"}},
        "required": ["n"],
    }

    @tool("add_one", namespace="math", input_schema=schema)
    def named(**kwargs) -> int:
        return kwargs["n"] + 1

    assert named.name == "add_one"
    assert named.namespace == "math"
    assert named.input_json_schema() == schema
    assert named.invoke(n=2) == 3


# ============================================================================
# SECTION: EXPLICIT output_schema (skips return-annotation inference)
# ============================================================================


def test_output_schema_dict_skips_inference() -> None:
    """An explicit JSON Schema dict for output is used as-is."""

    schema: dict = {"type": "integer", "minimum": 0}

    # Function annotation says `str`, but explicit dict overrides it.
    @tool("returns_int", output_schema=schema)
    def returns_int() -> str:
        return 42  # type: ignore[return-value]

    assert returns_int.output_json_schema() == schema
    assert returns_int.invoke() == 42


def test_output_schema_dict_rejects_invalid_output() -> None:
    """Dict-defined output_schema validates via jsonschema and rejects bad output."""

    @tool(
        "produces",
        output_schema={"type": "string", "minLength": 3},
    )
    def produces(value):
        return value

    assert produces.invoke(value="hello") == "hello"

    with pytest.raises(jsonschema.ValidationError):
        produces.invoke(value=123)  # not a string

    with pytest.raises(jsonschema.ValidationError):
        produces.invoke(value="hi")  # too short


def test_output_schema_dict_malformed_raises_at_construction() -> None:
    """A malformed output JSON Schema is rejected when the tool is built."""

    with pytest.raises(jsonschema.SchemaError):

        @tool("bad_out", output_schema={"type": "not-a-real-type"})
        def _bad_out() -> int:  # pyright: ignore[reportUnusedFunction]
            return 0


def test_output_schema_pydantic_model_skips_inference() -> None:
    """An explicit Pydantic BaseModel class for output is used as-is."""

    class Result(BaseModel):
        value: int
        label: str

    @tool("returns_model", output_schema=Result)
    def returns_model() -> dict:
        return {"value": 1, "label": "one"}

    schema = returns_model.output_json_schema()
    assert schema is not None
    assert schema["properties"] == {
        "value": {"title": "Value", "type": "integer"},
        "label": {"title": "Label", "type": "string"},
    }
    # TypeAdapter validates the dict against the Pydantic model.
    assert returns_model.invoke() == {"value": 1, "label": "one"}


def test_output_schema_pydantic_model_rejects_invalid_output() -> None:
    """Pydantic-defined output_schema raises pydantic.ValidationError on bad output."""

    class Result(BaseModel):
        value: int

    @tool("returns_model", output_schema=Result)
    def returns_model(payload):
        return payload

    with pytest.raises(ValidationError):
        returns_model.invoke(payload={"value": "not-an-int"})


def test_output_schema_overrides_return_annotation() -> None:
    """When output_schema is provided, the function's return annotation is ignored."""

    # Function annotated as -> int, but explicit schema says string.
    @tool("override", output_schema={"type": "string"})
    def override() -> int:
        return "hello"  # type: ignore[return-value]

    assert override.output_json_schema() == {"type": "string"}
    assert override.invoke() == "hello"


def test_output_schema_plain_python_type() -> None:
    """A plain python type (e.g. int) passed as output_schema works via TypeAdapter."""

    @tool("counter", output_schema=int)
    def counter(n):
        return n

    assert counter.output_json_schema() == {"type": "integer"}
    assert counter.invoke(n=5) == 5

    with pytest.raises(ValidationError):
        counter.invoke(n="not-an-int")


async def test_output_schema_dict_async() -> None:
    """Explicit output JSON Schema works for async tools too."""

    @tool("async_int", output_schema={"type": "integer"})
    async def async_int(n):
        return n

    assert isinstance(async_int, AsyncTool)
    assert await async_int.ainvoke(n=7) == 7

    with pytest.raises(jsonschema.ValidationError):
        await async_int.ainvoke(n="bad")


def test_input_and_output_schema_both_explicit() -> None:
    """Both schemas can be explicit dicts at once."""

    @tool(
        "echo",
        input_schema={
            "type": "object",
            "properties": {"msg": {"type": "string"}},
            "required": ["msg"],
        },
        output_schema={"type": "string"},
    )
    def echo(**kwargs) -> str:
        return kwargs["msg"]

    assert echo.input_json_schema() == {
        "type": "object",
        "properties": {"msg": {"type": "string"}},
        "required": ["msg"],
    }
    assert echo.output_json_schema() == {"type": "string"}
    assert echo.invoke(msg="hi") == "hi"

    with pytest.raises(jsonschema.ValidationError):
        echo.invoke(msg=123)  # input is not a string
