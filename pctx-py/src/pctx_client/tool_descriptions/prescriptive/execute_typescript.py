"""Prescriptive description for execute_typescript tool."""

DESCRIPTION = """Execute TypeScript code with access to registered SDK functions.

Your code runs in a TypeScript runtime with registered SDK functions already imported and available.

CODE STRUCTURE:
async function run() {
    // Your code here
    // Call Namespace.functionName() with proper types
    return result;
}

TOKEN USAGE WARNING:
- This tool could return LARGE responses if your code returns big objects
- Filter/map/reduce data IN YOUR CODE before returning
- Only return specific fields you need
- Use console.log() for intermediate results

IMPORTANT RULES:
- Functions MUST be called as 'Namespace.functionName'
- Only registered SDK functions are available
- No fetch(), fs, or other Node/Deno APIs
- Variables don't persist between executions
- Code runs in an isolated Deno sandbox

RETURN TYPE NOTE:
- Function results are already parsed JavaScript objects, NOT JSON strings
- Do NOT call JSON.parse() on results
- Access properties directly (e.g., result.data)"""
