# Tool Description Variants

This directory contains different prompting styles for tool descriptions. Use these to experiment with different approaches to agent guidance.

## Structure

Tool descriptions are organized by style, with each tool in its own file for better readability:

```
tool_descriptions/
├── _base.py                    # Shared utilities (HAS_SEARCH detection)
├── __init__.py                 # Builds description dicts from individual files
├── README.md                   # This file
├── prescriptive/               # Prescriptive style (step-by-step workflows)
│   ├── __init__.py
│   ├── list_functions.py
│   ├── search_functions.py
│   ├── get_function_details.py
│   ├── execute.py
│   ├── execute_bash.py
│   └── execute_typescript.py
└── terminal/                   # Terminal style (exploratory, OpenClaw-inspired)
    ├── __init__.py
    ├── list_functions.py
    ├── search_functions.py
    ├── get_function_details.py
    ├── execute.py
    ├── execute_bash.py
    └── execute_typescript.py
```

Each tool file exports a single `DESCRIPTION` constant that contains the tool's description.

## Benchmarking

coming soon

## Adding New Styles

To add a new description style:

1. Create a new directory `your_style/` with `__init__.py`
2. Create individual tool files (e.g., `execute.py`, `list_functions.py`)
3. Each file should export a `DESCRIPTION` constant
4. Update `tool_descriptions/__init__.py` to build `YOUR_STYLE_DESCRIPTIONS` dict
5. Add to `__all__` exports

Example:
```
your_style/
├── __init__.py
├── execute.py              # exports DESCRIPTION
├── list_functions.py       # exports DESCRIPTION
└── ...
```

## Usage

```python
from pctx_client.tool_descriptions import TERMINAL_STYLE_DESCRIPTIONS

# Use a built-in style
tools = pctx.langchain_tools(descriptions=TERMINAL_STYLE_DESCRIPTIONS)

# Or write your own
custom = {"execute": "Your description", "list_functions": "Another"}
tools = pctx.langchain_tools(descriptions=custom)

# Combine with different modes
tools = pctx.langchain_tools("fs", descriptions={"execute_bash": "Custom bash"})
```
