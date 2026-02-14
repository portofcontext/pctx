"""Terminal-style description for execute tool."""

DESCRIPTION = """Execute TypeScript code in an isolated Deno sandbox.

Your code runs in a TypeScript runtime with access to registered SDK functions.
Call functions as 'Namespace.functionName()' - they're already imported and available.

Runtime environment:
- Isolated Deno sandbox with restricted network access
- No Node.js/Deno APIs (fs, fetch, etc.) - only registered SDK functions
- Variables don't persist between executions
- Return values are automatically serialized

Code structure:
async function run() {
    // Your code here
    return yourResult;
}

Performance: 
- Large return values consume tokens
- Filter/reduce data in your code before returning. Use console.log() for debugging output
- Don't write any comments in your code"""
