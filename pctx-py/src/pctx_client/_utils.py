import re
from importlib.util import find_spec

HAS_SEARCH = find_spec("bm25s") is not None and find_spec("Stemmer") is not None


def to_snake_case(name: str) -> str:
    """Convert CamelCase to snake_case."""
    name = re.sub("(.)([A-Z][a-z]+)", r"\1_\2", name)
    name = re.sub("([a-z0-9])([A-Z])", r"\1_\2", name)
    return name.lower()
