"""Tests for create_output_schema"""

from typing import Annotated, get_args, get_origin

from pydantic import BaseModel, TypeAdapter

from pctx_client._tool import create_output_schema


def _unwrap_annotated(typ):
    """Helper to unwrap Annotated types"""
    if get_origin(typ) is Annotated:
        return get_args(typ)[0]
    return typ


def test_output_schema_simple_type():
    """Test creating output schema from simple return type"""

    def returns_int() -> int:
        return 42

    typ = create_output_schema(returns_int)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"type": "integer"}


def test_output_schema_string_type():
    """Test creating output schema from string return type"""

    def returns_str() -> str:
        return "hello"

    typ = create_output_schema(returns_str)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"type": "string"}


def test_output_schema_complex_type():
    """Test creating output schema from complex return type"""

    def returns_list() -> list[str]:
        return ["a", "b", "c"]

    typ = create_output_schema(returns_list)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"type": "array", "items": {"type": "string"}}


def test_output_schema_dict_type():
    """Test creating output schema from dict return type"""

    def returns_dict() -> dict[str, int]:
        return {"a": 1, "b": 2}

    typ = create_output_schema(returns_dict)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {
        "type": "object",
        "additionalProperties": {"type": "integer"},
    }


def test_output_schema_no_annotation():
    """Test creating output schema when no return annotation exists"""

    def no_return_type():
        return "something"

    typ = create_output_schema(no_return_type)
    adapter = TypeAdapter(typ)
    # Should use Any type - schema is empty dict for Any
    assert adapter.json_schema() == {}


def test_output_schema_with_pydantic_model():
    """Test that Pydantic model return types are used as-is"""

    class UserOutput(BaseModel):
        """User data from the database"""

        name: str
        age: int

    def returns_model() -> UserOutput:
        return UserOutput(name="Alice", age=30)

    typ = create_output_schema(returns_model)

    # Should be the same type (possibly wrapped in Annotated)
    assert _unwrap_annotated(typ) is UserOutput

    # Can use with TypeAdapter
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {
        "type": "object",
        "required": ["name", "age"],
        "title": "UserOutput",
        "description": "User data from the database",
        "properties": {
            "age": {
                "title": "Age",
                "type": "integer",
            },
            "name": {
                "title": "Name",
                "type": "string",
            },
        },
    }


def test_output_schema_with_nested_pydantic_model():
    """Test output schema when function returns nested Pydantic model"""

    class Address(BaseModel):
        street: str
        city: str

    class Person(BaseModel):
        name: str
        address: Address

    def returns_person() -> Person:
        return Person(name="Alice", address=Address(street="Main St", city="NYC"))

    typ = create_output_schema(returns_person)

    # Should return Person as-is (possibly wrapped in Annotated)
    assert _unwrap_annotated(typ) is Person
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {
        "type": "object",
        "required": ["name", "address"],
        "title": "Person",
        "properties": {
            "name": {
                "title": "Name",
                "type": "string",
            },
            "address": {
                "$ref": "#/$defs/Address",
            },
        },
        "$defs": {
            "Address": {
                "properties": {
                    "city": {
                        "title": "City",
                        "type": "string",
                    },
                    "street": {
                        "title": "Street",
                        "type": "string",
                    },
                },
                "required": [
                    "street",
                    "city",
                ],
                "title": "Address",
                "type": "object",
            },
        },
    }


def test_output_schema_optional_type():
    """Test creating output schema from optional return type"""

    def returns_optional() -> str | None:
        return None

    typ = create_output_schema(returns_optional)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"anyOf": [{"type": "string"}, {"type": "null"}]}


def test_output_schema_union_type():
    """Test creating output schema from union return type"""

    def returns_union() -> int | str:
        return 42

    typ = create_output_schema(returns_union)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"anyOf": [{"type": "integer"}, {"type": "string"}]}


def test_output_schema_bool_type():
    """Test creating output schema from bool return type"""

    def returns_bool() -> bool:
        return True

    typ = create_output_schema(returns_bool)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"type": "boolean"}


def test_output_schema_async_function():
    """Test creating output schema from async function"""

    async def async_returns_str() -> str:
        return "async result"

    typ = create_output_schema(async_returns_str)
    adapter = TypeAdapter(typ)
    assert adapter.json_schema() == {"type": "string"}
