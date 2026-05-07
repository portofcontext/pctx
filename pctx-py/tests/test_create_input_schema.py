"""Tests for infer_input_model"""

import pytest
from pydantic import BaseModel, ValidationError

from pctx_client._tool import infer_input_model


def test_simple_function_signature():
    """Test creating model from simple function signature"""

    def add(a: int, b: int) -> int:
        return a + b

    Model = infer_input_model("AddModel", add)

    # Test valid input
    instance = Model(a=5, b=10)
    assert getattr(instance, "a") == 5
    assert getattr(instance, "b") == 10

    # Test missing required field
    with pytest.raises(ValidationError):
        Model(a=5)

    # Test invalid types
    with pytest.raises(ValidationError):
        Model(a="foo", b="bar")


def test_function_with_defaults():
    """Test creating model from function with default values"""

    def greet(name: str, greeting: str = "Hello") -> str:
        return f"{greeting}, {name}!"

    Model = infer_input_model("GreetModel", greet)

    # Test with all parameters
    instance1 = Model(name="Alice", greeting="Hi")
    assert getattr(instance1, "name") == "Alice"
    assert getattr(instance1, "greeting") == "Hi"

    # Test with only required parameter
    instance2 = Model(name="Bob")
    assert getattr(instance2, "name") == "Bob"
    assert getattr(instance2, "greeting") == "Hello"


def test_function_without_type_hints():
    """Test creating model from function without type hints"""

    def no_types(x, y=5):
        return x + y

    Model = infer_input_model("NoTypesModel", no_types)

    # Should work with Any type
    instance = Model(x="test", y=10)
    assert getattr(instance, "x") == "test"
    assert getattr(instance, "y") == 10


def test_function_with_args_kwargs():
    """Test that *args and **kwargs are skipped"""

    def flexible(a: int, *args, b: str = "default", **kwargs) -> None:
        pass

    Model = infer_input_model("FlexibleModel", flexible)

    # Only 'a' and 'b' should be in the model
    instance = Model(a=42, b="test")
    assert getattr(instance, "a") == 42
    assert getattr(instance, "b") == "test"

    # Verify schema doesn't include args/kwargs
    schema = Model.model_json_schema()
    assert "args" not in schema["properties"]
    assert "kwargs" not in schema["properties"]
    assert set(schema["properties"].keys()) == {"a", "b"}

    # Should not accept arbitrary fields (extra='forbid')
    with pytest.raises(ValidationError):
        Model(a=42, b="test", extra_field="should fail")


def test_function_with_only_args():
    """Test function with only *args parameter"""

    def only_args(*args: int) -> None:
        pass

    Model = infer_input_model("OnlyArgsModel", only_args)

    # Model should have no fields
    schema = Model.model_json_schema()
    assert len(schema["properties"]) == 0

    # Should create empty instance
    instance = Model()
    assert instance is not None


def test_function_with_only_kwargs():
    """Test function with only **kwargs parameter"""

    def only_kwargs(**kwargs: str) -> None:
        pass

    Model = infer_input_model("OnlyKwargsModel", only_kwargs)

    # Model should have no fields
    schema = Model.model_json_schema()
    assert len(schema["properties"]) == 0

    # Should create empty instance
    instance = Model()
    assert instance is not None


def test_function_with_positional_only_and_args():
    """Test function with positional-only params and *args"""

    def mixed(a: int, b: str, /, c: float = 1.0, *args) -> None:
        pass

    Model = infer_input_model("MixedModel", mixed)

    # Should include regular params but not args
    schema = Model.model_json_schema()
    assert set(schema["properties"].keys()) == {"a", "b", "c"}
    assert "args" not in schema["properties"]
    assert set(schema["required"]) == {"a", "b"}

    # Test instantiation
    instance = Model(a=10, b="test", c=2.5)
    assert getattr(instance, "a") == 10
    assert getattr(instance, "b") == "test"
    assert getattr(instance, "c") == 2.5


def test_complex_types():
    """Test with complex type annotations"""

    def process(items: list[str], count: int = 0) -> None:
        pass

    Model = infer_input_model("ProcessModel", process)

    instance = Model(items=["a", "b", "c"], count=3)
    assert getattr(instance, "items") == ["a", "b", "c"]
    assert getattr(instance, "count") == 3

    # Test validation
    with pytest.raises(ValidationError):
        Model(items="not a list")


def test_model_is_basemodel():
    """Test that created model is a proper BaseModel"""

    def dummy(x: int) -> None:
        pass

    Model = infer_input_model("DummyModel", dummy)

    assert issubclass(Model, BaseModel)
    assert getattr(Model, "__name__") == "DummyModel"


def test_model_json_schema():
    """Test that the model can generate JSON schema"""

    def example(name: str, age: int, active: bool = True) -> None:
        pass

    Model = infer_input_model("ExampleModel", example)

    schema = Model.model_json_schema()
    assert "properties" in schema
    assert "name" in schema["properties"]
    assert "age" in schema["properties"]
    assert "active" in schema["properties"]
    assert schema["required"] == ["name", "age"]


def test_async_function_signature():
    """Test creating model from async function signature"""

    async def fetch_data(url: str, timeout: int = 30) -> str:
        """Fetches data from a URL"""
        return f"Data from {url}"

    Model = infer_input_model("FetchModel", fetch_data)

    # Test with all parameters
    instance1 = Model(url="https://example.com", timeout=60)
    assert getattr(instance1, "url") == "https://example.com"
    assert getattr(instance1, "timeout") == 60

    # Test with only required parameter
    instance2 = Model(url="https://test.com")
    assert getattr(instance2, "url") == "https://test.com"
    assert getattr(instance2, "timeout") == 30

    # Test validation
    with pytest.raises(ValidationError):
        Model(timeout=30)  # Missing required 'url'


def test_nested_pydantic_model():
    """Test creating model with nested Pydantic model as input type"""

    class Address(BaseModel):
        street: str
        city: str
        zipcode: str

    class ContactInfo(BaseModel):
        email: str
        phone: str | None = None

    def create_user(
        name: str, address: Address, contact: ContactInfo, age: int = 18
    ) -> None:
        pass

    Model = infer_input_model("UserModel", create_user)

    # Test with valid nested models
    address_data = Address(street="123 Main St", city="Boston", zipcode="02101")
    contact_data = ContactInfo(email="test@example.com", phone="555-1234")

    instance = Model(name="Alice", address=address_data, contact=contact_data, age=25)

    assert getattr(instance, "name") == "Alice"
    assert getattr(instance, "address") == address_data
    assert getattr(instance, "contact") == contact_data
    assert getattr(instance, "age") == 25

    # Test with dict input (Pydantic auto-converts)
    instance2 = Model(
        name="Bob",
        address={"street": "456 Oak Ave", "city": "NYC", "zipcode": "10001"},
        contact={"email": "bob@test.com"},
    )

    assert getattr(instance2, "name") == "Bob"
    assert isinstance(getattr(instance2, "address"), Address)
    assert getattr(instance2, "address").city == "NYC"
    assert isinstance(getattr(instance2, "contact"), ContactInfo)
    assert getattr(instance2, "contact").phone is None

    # Test validation of nested model
    with pytest.raises(ValidationError):
        Model(
            name="Charlie",
            address={"street": "789 Pine Rd"},  # Missing required fields
            contact={"email": "charlie@test.com"},
        )


def test_tuple_type_annotation():
    """Test creating model with tuple type annotations"""

    def process_coordinates(
        point: tuple[int, int], color: tuple[int, int, int] = (255, 255, 255)
    ) -> None:
        pass

    Model = infer_input_model("CoordinatesModel", process_coordinates)

    # Test with valid tuples
    instance1 = Model(point=(10, 20), color=(100, 150, 200))
    assert getattr(instance1, "point") == (10, 20)
    assert getattr(instance1, "color") == (100, 150, 200)

    # Test with default value
    instance2 = Model(point=(5, 15))
    assert getattr(instance2, "point") == (5, 15)
    assert getattr(instance2, "color") == (255, 255, 255)

    # Test validation - wrong number of elements
    with pytest.raises(ValidationError):
        Model(point=(10, 20, 30))  # Should be 2 elements

    # Test validation - wrong types
    with pytest.raises(ValidationError):
        Model(point=("a", "b"))
