"""Terminal-style description for execute_typescript tool."""

DESCRIPTION = """Execute TypeScript code with SDK functions.

Your code runs in a Deno sandbox with access to SDK functions discovered via execute_bash.
Functions are called as 'Namespace.functionName()' and return parsed JavaScript objects.

Runtime:
- Isolated Deno sandbox (no fs, fetch, or other APIs)
- Variables don't persist between runs
- Return values are serialized automatically

Structure:
async function run() {
    // Your TypeScript code
    return result;
}

Keep return values small to minimize token usage."""
