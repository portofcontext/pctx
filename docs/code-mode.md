# Code Mode Interface

**`pctx` is a bring-your-own-LLM proxy** that exposes MCP tools as TypeScript functions for AI agents.

## What is Code Mode?

Instead of sequential tool calls passing data through the model's context window, code mode lets AI agents write TypeScript that executes in a sandboxed environment.

**Traditional MCP:** Each tool call → model context → high token usage
**Code Mode:** Write TypeScript → execute locally → 98% fewer tokens

```typescript
// Data stays in execution environment, process with native JS
const items = await myserver.getItems();
const filtered = items.filter(x => x.status === 'pending');
await myserver.updateItems(filtered.map(x => ({ ...x, processed: true })));
```

## How It Works

```
1. AI discovers → pctx.list_functions()
2. AI gets details → pctx.get_function_details(['gdrive.getSheet'])
3. AI writes code → TypeScript using discovered functions
4. pctx type-checks → Instant feedback (< 100ms)
5. pctx executes → Deno sandbox if types pass
6. AI gets results → stdout + return value
```

### Type Checking Before Execution

`pctx` validates TypeScript **before running code** using `typescript-go`:

```typescript
// ✓ Valid
await gdrive.getSheet({ sheetId: 'abc123' });

// ✗ Type errors caught instantly
await gdrive.getSheet({ sheetId: 123 });
// Error: Type 'number' is not assignable to type 'string'
```

**10-20x faster iteration** - No execution overhead for type errors.

### Sandboxed Execution

Code runs in Deno with strict limits:
- **10-second timeout**
- **No filesystem/env access**
- **Network restricted** to configured MCP hosts only
- **Pre-authenticated** MCP clients (AI never sees credentials)

## Benefits

### 98% Token Reduction
- **Traditional:** 150K tokens (data → model → data → model)
- **Code mode:** 2K tokens (single code block, process in sandbox)

### Faster Execution
- Write code once, execute locally
- Use native JS: `filter`, `map`, `reduce`, loops, conditionals
- Handle errors with try/catch

### Type Safety
- Instant feedback (< 100ms)
- Prevent invalid code from running
- Clear error messages with line/column

## Three MCP Tools

`pctx` exposes three tools that your LLM calls:

### 1. `list_functions`
Returns TypeScript namespaces for all connected MCP servers.

### 2. `get_function_details`
Returns full TypeScript signatures with JSDoc for specific functions.

### 3. `execute`
Runs TypeScript code with type checking, returns `{ success, stdout, output, diagnostics }`.

**Typical flow:**
```
list_functions() → get_function_details([...]) → execute({ code })
```

## Namespaces

Each MCP server becomes a TypeScript namespace:

```typescript
// Server names from config
await gdrive.getSheet({ sheetId: 'abc' });
await slack.sendMessage({ channel: '#general', text: 'hi' });
```

## Example

```typescript
// Traditional: 50K+ tokens for multiple tool calls
// Code mode: Single execution, data stays in sandbox
const orders = await store.getOrders();
const pending = orders.filter(o => o.status === 'pending');
const total = pending.reduce((sum, o) => sum + o.amount, 0);
console.log(`${pending.length} pending orders: $${total}`);
```

## Learn More

- [Upstream MCP Servers](upstream-mcp-servers.md) - Connect multiple MCP servers
- [MCP Authentication](mcp-auth.md) - Secure credential management
- [Model Context Protocol](https://modelcontextprotocol.io/) - MCP specification
