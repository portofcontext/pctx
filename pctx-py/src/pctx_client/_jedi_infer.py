"""
Optional Jedi-based return type inference.

This module provides functionality to infer return types from functions
that don't have explicit type annotations, using Jedi's static analysis.

Requires the jedi optional dependency: pip install pctx-client[jedi]
"""

from __future__ import annotations

import ast
import inspect
import sys
import typing
from collections.abc import Callable
from functools import lru_cache
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import jedi

# Optional dependency check
try:
    import jedi as _jedi

    HAS_JEDI = True
except ImportError:
    HAS_JEDI = False
    _jedi = None  # type: ignore


class JediInferenceError(Exception):
    """Raised when Jedi inference fails."""

    pass


@lru_cache(maxsize=32)
def _get_jedi_script(source_path: str) -> "jedi.Script":
    """Get cached Jedi Script for a source file."""
    if not HAS_JEDI:
        raise JediInferenceError("Jedi is not installed")

    with open(source_path) as f:
        source_code = f.read()

    return _jedi.Script(source_code, path=source_path)


def _build_type_namespace(func: Callable) -> dict[str, Any]:
    """Build namespace for evaluating type strings."""
    namespace: dict[str, Any] = {
        # Built-in types
        "dict": dict,
        "list": list,
        "set": set,
        "tuple": tuple,
        "str": str,
        "int": int,
        "float": float,
        "bool": bool,
        "bytes": bytes,
        "None": type(None),
        "type": type,
        # Typing constructs
        **vars(typing),
    }

    # Add types from the function's module
    if hasattr(func, "__module__") and func.__module__ in sys.modules:
        module = sys.modules[func.__module__]
        for name, obj in vars(module).items():
            if isinstance(obj, type):
                namespace[name] = obj
            # Also include TypedDict, generic aliases, etc.
            elif hasattr(obj, "__origin__") or (
                hasattr(obj, "__class__") and "TypedDict" in str(type(obj))
            ):
                namespace[name] = obj

    return namespace


def _find_function_position(source_code: str, func_name: str) -> tuple[int, int] | None:
    """Find the line and column of a function definition using AST."""
    try:
        tree = ast.parse(source_code)
        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                if node.name == func_name:
                    # Column should point to the function name, after 'def '
                    col = node.col_offset + len("def ") + 1
                    return node.lineno, col
    except SyntaxError:
        pass
    return None


def _parse_return_type_from_hint(type_hint: str) -> str:
    """Extract return type from Jedi's type hint string like '(x: int) -> str'."""
    if " -> " in type_hint:
        return type_hint.split(" -> ", 1)[1].strip()
    return "Any"


def infer_return_type(func: Callable) -> Any:
    """
    Infer the return type of a function using Jedi static analysis.

    Args:
        func: The function to analyze

    Returns:
        The inferred return type, or Any if inference fails

    Raises:
        JediInferenceError: If Jedi is not installed
    """
    if not HAS_JEDI:
        raise JediInferenceError(
            "Jedi is not installed. Install it with: pip install pctx-client[jedi]"
        )

    # Get source file info
    try:
        source_path = inspect.getsourcefile(func)
        # print(f"Source Path: {source_path}")
        if source_path is None:
            return Any  # Built-in or dynamic function
    except TypeError:
        return Any  # Built-in function

    # Get function name
    func_name = getattr(func, "__name__", None)
    # print(f"Func Name: {func_name}")
    if func_name is None:
        return Any  # Lambda or unnamed callable

    # Get Jedi script (cached)
    try:
        script = _get_jedi_script(source_path)
    except (OSError, JediInferenceError):
        return Any

    # Read source to find function position
    try:
        with open(source_path) as f:
            source_code = f.read()
    except OSError:
        return Any

    position = _find_function_position(source_code, func_name)
    # print(f"Position: {position}")
    if position is None:
        return Any

    line, col = position

    # Use Jedi to infer types
    try:
        names = script.infer(line, col)
        if not names:
            return Any

        # Get the type hint from the first result
        type_hint = names[0].get_type_hint()
        # print(f"type_hint: {type_hint}")
        if not type_hint:
            return Any

        return_type_str = _parse_return_type_from_hint(type_hint)
        # print(f"return_type_str: {return_type_str}")
        if return_type_str == "Any":
            return Any

        # Evaluate the type string
        namespace = _build_type_namespace(func)
        # print(f"namespace: {namespace}")
        # eval_res = eval(return_type_str, namespace)
        # print(f"eval_res: {eval_res}")
        return eval(return_type_str, namespace)

    except Exception:
        # Any failure in Jedi inference should fall back gracefully
        return Any


def clear_cache() -> None:
    """Clear the Jedi script cache. Useful for long-running processes."""
    _get_jedi_script.cache_clear()
