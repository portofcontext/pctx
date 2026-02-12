"""Prescriptive description for get_function_details tool."""

DESCRIPTION = """Get detailed TypeScript type information for specific SDK functions you want to use.

REQUIRED FORMAT: Functions must be specified as 'namespace.functionName' (e.g., 'Namespace.apiPostSearch')

This tool is lightweight and only returns details for the functions you request, avoiding unnecessary token usage.
Only request details for functions you actually plan to use in your code.

NOTE ON RETURN TYPES:
- If a function returns Promise<any>, the tool didn't provide an output schema
- The actual value is a parsed object (not a string) - access properties directly
- Don't use JSON.parse() on the results - they're already JavaScript objects"""
