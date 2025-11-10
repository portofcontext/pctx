# PCTX Code Execution Runtime

A Deno extension providing MCP (Model Context Protocol) client functionality and console output capturing.

## Quick Start

```rust
use deno_core::{JsRuntime, RuntimeOptions};
use pctx_runtime::{pctx_runtime_snapshot, MCPRegistry, AllowedHosts, RUNTIME_SNAPSHOT};

// Create a new runtime with the PCTX extension
let registry = MCPRegistry::new();
let allowed_hosts = AllowedHosts::new(Some(vec!["example.com".to_string()]));

let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(RUNTIME_SNAPSHOT),
    extensions: vec![pctx_runtime_snapshot::init(registry, allowed_hosts)],
    ..Default::default()
});

// MCP API is now available in JavaScript
let code = r#"
    registerMCP({ name: "my-server", url: "http://localhost:3000" });

    const result = await callMCPTool({
        name: "my-server",
        tool: "get_data",
        arguments: { id: 42 }
    });

    console.log("Result:", result);
"#;

runtime.execute_script("<main>", code)?;
```

## Rust API Reference

### Core Types

#### `MCPRegistry`

Thread-safe registry for MCP server configurations.

```rust
let registry = MCPRegistry::new();
// Pass to extension initializer
```

#### `MCPServerConfig`

Configuration for an MCP server.

```rust
pub struct MCPServerConfig {
    pub name: String,
    pub url: String,
}
```

#### `AllowedHosts`

Whitelist of hosts allowed for network access.

```rust
let allowed_hosts = AllowedHosts::new(vec![
    "example.com".to_string(),
    "api.service.com".to_string(),
]);
```

### Snapshot

#### `RUNTIME_SNAPSHOT`

Pre-compiled V8 snapshot containing the runtime.

```rust
pub static RUNTIME_SNAPSHOT: &[u8] = /* ... */;
```

## Examples

### Console Output Capture

```rust
let code = r#"
    console.log("Line 1");
    console.log("Line 2");
    console.error("Error line");

    export default {
        stdout: globalThis.__stdout,
        stderr: globalThis.__stderr
    };
"#;

let result = runtime.execute_script("<capture>", code)?;

// Extract captured output
let scope = &mut runtime.handle_scope();
let local = v8::Local::new(scope, result);
let output = serde_v8::from_v8::<serde_json::Value>(scope, local)?;

println!("Stdout: {:?}", output["stdout"]);
println!("Stderr: {:?}", output["stderr"]);
```

### Network Permissions

```rust
// Allow only specific hosts
let allowed_hosts = AllowedHosts::new(Some(vec![
    "api.example.com".to_string(),
    "cdn.example.com".to_string(),
]));

let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(RUNTIME_SNAPSHOT),
    extensions: vec![pctx_runtime_snapshot::init(
        MCPRegistry::new(),
        allowed_hosts
    )],
    ..Default::default()
});

let code = r#"
    // This will succeed
    await fetch("http://api.example.com/data");

    // This will fail - host not allowed
    try {
        await fetch("http://malicious.com/data");
    } catch (e) {
        console.error("Blocked:", e.message);
    }
"#;

runtime.execute_script("<permissions>", code)?;
```


## Security

### Network Access

- Only whitelisted hosts can be accessed via `fetch()`
- Attempts to access non-whitelisted hosts throw errors
- Host matching is exact (no wildcards)

### MCP Registry

- Each runtime instance has its own isolated registry
- No cross-runtime access to MCP configurations
- Registry is not persisted between runtime sessions

### Console Capture

- Captured output is stored in runtime-local buffers
- No disk I/O or external logging
- Buffers cleared when runtime is dropped

## Performance

- **Startup**: Instant (V8 snapshot loads in <1ms)
- **Memory**: ~2MB base runtime overhead
- **MCP Operations**: Native Rust performance
- **Console Capture**: Minimal overhead (~1% per log)

## TypeScript Definitions

The runtime provides full TypeScript type definitions:

```typescript
interface MCPServerConfig {
    name: string;
    url: string;
}

interface MCPToolCall {
    name: string;
    tool: string;
    arguments?: any;
}

declare function registerMCP(config: MCPServerConfig): void;
declare function callMCPTool<T = any>(call: MCPToolCall): Promise<T>;

declare const REGISTRY: {
    has(name: string): boolean;
    get(name: string): MCPServerConfig | undefined;
    delete(name: string): boolean;
    clear(): void;
};

declare function fetch(url: string, init?: any): Promise<Response>;
```

## License

MIT

## Contributing

Contributions welcome! Please ensure:
- All tests pass: `cargo test --package pctx_runtime`
- Code is formatted: `cargo fmt`
- Documentation is updated

## See Also

- [`pctx_type_check`](../pctx_type_check) - TypeScript type checking runtime
- [`deno_executor`](../deno_executor) - Complete TypeScript execution environment
- [Model Context Protocol](https://modelcontextprotocol.io) - MCP specification
