"""Prescriptive description for execute tool."""

DESCRIPTION = """Execute TypeScript code that calls namespaced functions.

TOKEN USAGE WARNING: This tool could return LARGE responses if your code returns big objects.
To minimize tokens:
- Filter/map/reduce data IN YOUR CODE before returning
- Only return specific fields you need (e.g., return {id: result.id, count: items.length})
- Use console.log() for intermediate results instead of returning everything
- Avoid returning full API responses - extract just what you need

REQUIRED CODE STRUCTURE:
async function run() {
    // Your code here
    // Call namespace.functionName() - MUST include namespace prefix
    // Process data here to minimize return size
    return onlyWhatYouNeed; // Keep this small!
}

IMPORTANT RULES:
- Functions MUST be called as 'Namespace.functionName' (e.g., 'Notion.apiPostSearch')
- Only registered SDK functions are available - no fetch(), fs, or other Node/Deno APIs
- Variables don't persist between execute() calls - return or log anything you need later
- Add console.log() statements between API calls to track progress if errors occur
- Code runs in an isolated Deno sandbox with restricted network access

RETURN TYPE NOTE:
- Functions without output schemas show Promise<any> as return type
- The actual runtime value is already a parsed JavaScript object, NOT a JSON string
- Do NOT call JSON.parse() on results - they're already objects
- Access properties directly (e.g., result.data) or inspect with console.log() first
- If you see 'Promise<any>', the structure is unknown - log it to see what's returned"""
