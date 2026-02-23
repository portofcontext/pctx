"""Terminal-style description for execute_python tool."""

DESCRIPTION = """Execute Python code that calls the available tools as functions.

Programmatic Tool Calling lets you orchestrate complex multi-tool workflows through code — sequencing calls, transforming data between them, and deciding exactly what enters your context window. This eliminates inference round-trips between tool calls and significantly reduces token usage compared to calling tools one at a time.

Call tools directly by function name (snake_case of the tool name). The last expression is returned. Only registered tools are available — no imports, no filesystem, no network access, no persistent state between calls."""
