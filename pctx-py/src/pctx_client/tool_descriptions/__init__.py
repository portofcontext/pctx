"""Tool description variants for experimentation with different prompting styles.

This module builds description dictionaries from individual tool files organized by style.
"""

from . import prescriptive, terminal, workflows

# Build description dictionaries from individual tool modules
PRESCRIPTIVE_DESCRIPTIONS = {
    "list_functions": prescriptive.list_functions.DESCRIPTION,
    "search_functions": prescriptive.search_functions.DESCRIPTION,
    "get_function_details": prescriptive.get_function_details.DESCRIPTION,
    "execute": prescriptive.execute.DESCRIPTION,
    "execute_bash": prescriptive.execute_bash.DESCRIPTION,
    "execute_typescript": prescriptive.execute_typescript.DESCRIPTION,
}

TERMINAL_STYLE_DESCRIPTIONS = {
    "list_functions": terminal.list_functions.DESCRIPTION,
    "search_functions": terminal.search_functions.DESCRIPTION,
    "get_function_details": terminal.get_function_details.DESCRIPTION,
    "execute": terminal.execute.DESCRIPTION,
    "execute_bash": terminal.execute_bash.DESCRIPTION,
    "execute_typescript": terminal.execute_typescript.DESCRIPTION,
}

__all__ = [
    "PRESCRIPTIVE_DESCRIPTIONS",
    "TERMINAL_STYLE_DESCRIPTIONS",
    "workflows",
]
