"""Tests for Jedi-based return type inference."""

from typing import Any, Dict, List

from pydantic import BaseModel

from pctx_client import tool
from pctx_client._tool import create_output_schema

# =============================================================================
# Pydantic models for testing
# =============================================================================


class UserModel(BaseModel):
    name: str
    age: int


class OrderModel(BaseModel):
    item: str
    quantity: int
    price: float


# =============================================================================
# Test functions defined at module level for Jedi to analyze
# =============================================================================


def _returns_string(x: str):
    return f"hello {x}"


def _returns_dict(x: int):
    return {"value": x}


def _returns_list(x: int):
    return [x, x + 1, x + 2]


def _has_annotation() -> str:
    return "hello"


def _no_annotation(x: str):
    return f"hello {x}"


def _has_annotation_with_param(x: str) -> str:
    return f"hello {x}"


def _simple_func(x: int):
    return x * 2


def _returns_pydantic_model(name: str, age: int):
    return UserModel(name=name, age=age)


def _returns_list_of_pydantic_models(count: int):
    return [
        OrderModel(item=f"item_{i}", quantity=i, price=i * 10.0) for i in range(count)
    ]


# =============================================================================
# Tests for Jedi inference
# =============================================================================


def test_infer_simple_string_return():
    """Test inferring simple string return type."""
    typ = create_output_schema(_returns_string, infer_with_jedi=True)
    assert typ is str


def test_infer_dict_return_type():
    """Test inferring dict return types."""
    typ = create_output_schema(_returns_dict, infer_with_jedi=True)
    assert typ == Dict[str, int]


def test_infer_list_return_type():
    """Test inferring list return types."""
    typ = create_output_schema(_returns_list, infer_with_jedi=True)
    assert typ == List[int]


def test_infer_pydantic_model_return():
    """Test inferring Pydantic model return type."""
    typ = create_output_schema(_returns_pydantic_model, infer_with_jedi=True)
    assert typ is UserModel


def test_infer_list_of_pydantic_models_return():
    """Test inferring list of Pydantic models return type."""
    typ = create_output_schema(_returns_list_of_pydantic_models, infer_with_jedi=True)
    # Should infer list[OrderModel] or similar
    assert typ == List[OrderModel]


def test_fallback_when_annotation_exists():
    """Test that explicit annotations take precedence."""
    typ = create_output_schema(_has_annotation, infer_with_jedi=True)
    assert typ is str


def test_builtin_function_returns_any():
    """Test that built-in functions gracefully return Any."""
    typ = create_output_schema(len, infer_with_jedi=True)
    assert typ is Any


def test_lambda_returns_any():
    """Test that lambdas gracefully return Any."""
    fn = lambda x: x + 1  # noqa: E731
    typ = create_output_schema(fn, infer_with_jedi=True)
    assert typ is Any


# =============================================================================
# Tests for default behavior (no inference)
# =============================================================================


def test_no_inference_by_default():
    """Test that inference is disabled by default."""
    typ = create_output_schema(_no_annotation, infer_with_jedi=False)
    assert typ is Any


def test_explicit_annotation_still_works():
    """Test that explicit annotations work without inference."""
    typ = create_output_schema(_has_annotation_with_param, infer_with_jedi=False)
    assert typ is str


# =============================================================================
# Tests for @tool decorator with infer_return_type parameter
# =============================================================================


@tool(infer_return_type=True)
def _my_tool_inferred(x: str):
    return f"processed: {x}"


@tool(infer_return_type=True)
def _dict_tool_inferred(x: int):
    return {"count": x}


@tool
def _my_tool_no_infer(x: str):
    return f"processed: {x}"


@tool(infer_return_type=True)
def _typed_tool(x: str) -> str:
    return f"typed: {x}"


def test_tool_decorator_with_inference():
    """Test @tool decorator with infer_return_type=True."""
    # Should have inferred output schema (str)
    assert _my_tool_inferred.output_schema is str


def test_tool_decorator_with_inference_dict():
    """Test @tool decorator inferring dict return."""
    assert _dict_tool_inferred.output_schema == Dict[str, int]


def test_tool_decorator_without_inference():
    """Test @tool decorator defaults to no inference."""
    # Output schema should be Any (no inference)
    assert _my_tool_no_infer.output_schema is Any


def test_tool_with_explicit_annotation():
    """Test @tool with explicit return annotation."""
    assert _typed_tool.output_schema is str


# =============================================================================
# Tests for the _jedi_infer module directly
# =============================================================================


def test_infer_return_type_function():
    """Test infer_return_type function directly."""
    from pctx_client._jedi_infer import infer_return_type

    typ = infer_return_type(_simple_func)
    assert typ is int


def test_clear_cache():
    """Test cache clearing doesn't raise."""
    from pctx_client._jedi_infer import clear_cache

    clear_cache()  # Should not raise


def test_has_jedi_flag():
    """Test HAS_JEDI flag is True when jedi is installed."""
    from pctx_client._jedi_infer import HAS_JEDI

    assert HAS_JEDI is True
