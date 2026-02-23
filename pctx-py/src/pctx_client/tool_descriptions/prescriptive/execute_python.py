"""Prescriptive description for execute_python tool."""

DESCRIPTION = """Execute Python code that calls the available tools as native Python functions.

Use it to eliminate round-trips between tightly coupled calls, where results chain directly into subsequent inputs or filters.

Don't use this tool when you need to learn about the state/results of other tools first. Only use it when you have a tight batch of typed tool calls to script up.

Tools are bare global functions — call them directly with keyword arguments: `tool(param=value)`. No imports, no stdlib, no persistent state. Write concise code — no comments, no unnecessary variable assignments.

RESTRICTIONS:
- No `import` or `from X import` — not available in this environment
- No `map()`, `filter()`, `next()` — use list comprehensions instead
- Never write comments, it's a waste of time and tokens

PATTERNS:
  Last expression returned:  result = get_customer(...); result
  Early exit with return:    cust = get_customer(...)\nif not cust: return "not found"\ncust["id"]
  Safe dict access:          cust["customer_id"]  not  cust.get("customer_id")
  List transform:            [x["id"] for x in items]  not  map(lambda x: x["id"], items)
  Numeric arithmetic:        float(usage["data_limit_gb"]) + float(usage["data_used_gb"])  — be sure to cast types before doing arithmetic"""
