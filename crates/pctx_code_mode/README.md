# PCTX Code Mode

A TypeScript code execution engine that enables AI agents to dynamically call tools through generated code. Code Mode converts tool schemas (like MCP tools) into TypeScript interfaces, executes LLM-generated code in a sandboxed Deno runtime, and bridges function calls back to your Rust callbacks.

## Quick Start

```rust
use pctx_code_mode::{CodeMode};
use pctx_code_mode::registry::PctxRegistry;
use pctx_code_mode::config::ToolDisclosure;
use pctx_code_mode::model::CallbackConfig;
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Define callback tools with JSON schemas
    let callback = CallbackConfig {
        namespace: "Greeter".to_string(),
        name: "greet".to_string(),
        description: Some("Greets a person by name".to_string()),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        })),
        output_schema: Some(json!({
            "type": "object",
            "properties": { "message": { "type": "string" } },
            "required": ["message"]
        })),
    };

    // 2. Create CodeMode instance and add callback
    let mut code_mode = CodeMode::default();
    code_mode.add_callback(&callback)?;

    // 3. Register callback functions that execute when tools are called
    let registry = PctxRegistry::default();
    registry.add_callback(&callback.id(), Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("World");
            Ok(serde_json::json!({ "message": format!("Hello, {name}!") }))
        })
    }));

    // 4. Execute LLM-generated TypeScript code
    let code = r#"
        async function run() {
            const result = await Greeter.greet({ name: "Alice" });
            return result;
        }
    "#;

    let output = code_mode.execute_typescript(code, ToolDisclosure::default(), registry).await?;

    if output.success {
        println!("Result: {}", serde_json::to_string_pretty(&output.output)?);
    } else {
        eprintln!("Error: {}", output.stderr);
    }

    Ok(())
}
```

## Core Concepts

### 1. CodeMode

The `CodeMode` struct is the main execution engine. It provides:

**Builder methods** (chainable):

- `with_server()` / `with_servers()` - Add MCP servers
- `with_callback()` / `with_callbacks()` - Add callback tools

**Registration methods** (mutable):

- `add_server()` / `add_servers()` - Add MCP servers
- `add_callback()` / `add_callbacks()` - Add callback tools
- `add_tool_set()` - Add a pre-built ToolSet directly

**Accessor methods**:

- `tool_sets()` - Get all registered ToolSets
- `server_tool_sets()` - Get only MCP server ToolSets
- `servers()` - Get registered server configurations
- `callbacks()` - Get registered callback configurations
- `virtual_fs()` - Get the virtual filesystem used by bash execution

**Execution methods**:

- `list_functions()` - List all available functions with minimal interfaces
- `get_function_details()` - Get full typed interfaces for specific functions
- `execute_typescript()` - Execute TypeScript code in the sandbox
- `execute_bash()` - Execute a bash command in the virtual filesystem

```rust
use pctx_code_mode::CodeMode;
use pctx_code_mode::model::{CallbackConfig, GetFunctionDetailsInput, FunctionId};
use serde_json::json;

let mut code_mode = CodeMode::default();

// Add callback tools
code_mode.add_callback(&CallbackConfig {
    namespace: "DataApi".to_string(),
    name: "fetchData".to_string(),
    description: Some("Fetches data from API".to_string()),
    input_schema: Some(json!({
        "type": "object",
        "properties": { "id": { "type": "integer" } },
        "required": ["id"]
    })),
    output_schema: None,
})?;

// List available functions
let list = code_mode.list_functions();
for func in list.functions {
    println!("{}.{}: {:?}", func.namespace, func.name, func.description);
}

// Get detailed type information
let details = code_mode.get_function_details(GetFunctionDetailsInput {
    functions: vec![
        FunctionId { mod_name: "DataApi".into(), fn_name: "fetchData".into() }
    ],
});
println!("TypeScript definitions:\n{}", details.code);
```

### 2. ToolDisclosure

`ToolDisclosure` controls how tools are presented to the LLM and how generated TypeScript code invokes them. Choose the mode that matches your agent's workflow:

- **`Catalog`** (default) - Tools are discovered via `list_tools` → `get_tool_details`, then called through typed TypeScript namespaces (e.g. `await Greeter.greet({ name: "Alice" })`).
- **`Filesystem`** - Like `Catalog` but the agent works within a virtual filesystem via `execute_bash` before invoking TypeScript.
- **`Sidecar`** - Tools are passed as original MCP descriptions. The generated code uses an `InvokeMap` type and a type-safe `invoke()` function rather than namespace methods.

```rust
use pctx_code_mode::config::ToolDisclosure;

// Default catalog mode — typed namespaces
let output = code_mode.execute_typescript(code, ToolDisclosure::Catalog, registry).await?;

// Sidecar mode — InvokeMap / invoke() interface
let output = code_mode.execute_typescript(code, ToolDisclosure::Sidecar, registry).await?;
```

### 3. Tools and ToolSets

`Tool`s represent individual functions callable from TypeScript.
They are organized into `ToolSet`s (namespaces). Tools can be:

- **MCP tools**: Loaded from MCP servers via `add_server()`
- **Callback tools**: Defined via `CallbackConfig` and `add_callback()`

### 4. PctxRegistry

`PctxRegistry` is a thread-safe registry that routes TypeScript function calls to either local Rust callbacks or upstream MCP servers. Pass it to `execute_typescript()`.

```rust
use pctx_code_mode::{PctxRegistry, CallbackFn};
use std::sync::Arc;

let registry = PctxRegistry::default();

let callback: CallbackFn = Arc::new(|args| {
    Box::pin(async move {
        // Extract arguments
        let id = args
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_i64())
            .ok_or("Missing id")?;

        // Do async work
        let data = fetch_from_database(id).await?;

        // Return JSON result
        Ok(serde_json::to_value(data)?)
    })
});

// Register with namespace.function format
registry.add_callback("DataApi.fetchData", callback);

// Register MCP tools from a server
registry.add_mcp(tool_names, server_config);
```

### 5. Code Execution

Execute LLM-generated TypeScript code that calls your registered tools.

```rust
use pctx_code_mode::config::ToolDisclosure;

let code = r#"
    async function run() {
        // Call your registered tools
        const user = await DataApi.fetchData({ id: 123 });
        const greeting = await Greeter.greet({ name: user.name });

        // Chain multiple calls
        const result = await DataApi.saveData({
            id: user.id,
            message: greeting.message
        });

        // Return the final result
        return result;
    }
"#;

let output = code_mode.execute_typescript(code, ToolDisclosure::default(), registry).await?;

match output.success {
    true => println!("Success: {:?}", output.output),
    false => eprintln!("Error: {}", output.stderr),
}
```

## API Reference

### CodeMode

The main execution engine.

#### `default()`

```rust
let code_mode = CodeMode::default();
```

#### Builder Methods

Chainable methods for fluent construction:

```rust
use pctx_code_mode::CodeMode;
use pctx_code_mode::model::CallbackConfig;
use pctx_config::server::ServerConfig;

// Build with callbacks
let code_mode = CodeMode::default()
    .with_callback(&callback_config)?
    .with_callbacks(&[callback1, callback2])?;

// Build with MCP servers (async)
let code_mode = CodeMode::default()
    .with_server(&server_config).await?
    .with_servers(&server_configs, 30).await?;
```

#### `add_callback(config: &CallbackConfig) -> Result<()>`

Adds a callback-based tool to the code mode.

```rust
use pctx_code_mode::model::CallbackConfig;
use serde_json::json;

code_mode.add_callback(&CallbackConfig {
    namespace: "Logger".to_string(),
    name: "logMessage".to_string(),
    description: Some("Logs a message".to_string()),
    input_schema: Some(json!({
        "type": "object",
        "properties": {
            "message": { "type": "string" }
        },
        "required": ["message"]
    })),
    output_schema: None,
})?;
```

#### `add_server(server: &ServerConfig) -> Result<()>`

Connects to an MCP server and registers its tools.

```rust
use pctx_config::server::ServerConfig;

code_mode.add_server(&server_config).await?;

// Or add multiple servers with a timeout (in seconds)
code_mode.add_servers(&server_configs, 30).await?;
```

#### `list_functions() -> ListFunctionsOutput`

Lists all available functions with their TypeScript interface declarations.

```rust
let list = code_mode.list_functions();
println!("Available functions:\n{}", list.code);
for func in list.functions {
    println!("  {}.{}", func.namespace, func.name);
}
```

#### `get_function_details(input: GetFunctionDetailsInput) -> GetFunctionDetailsOutput`

Gets detailed TypeScript type definitions for specific functions.

```rust
use pctx_code_mode::model::{GetFunctionDetailsInput, FunctionId};

let details = code_mode.get_function_details(GetFunctionDetailsInput {
    functions: vec![
        FunctionId {
            mod_name: "DataApi".to_string(),
            fn_name: "fetchData".to_string(),
        }
    ],
});

println!("TypeScript code:\n{}", details.code);
```

#### `execute_typescript(code: &str, disclosure: ToolDisclosure, registry: PctxRegistry) -> Result<ExecuteOutput>`

Executes TypeScript code in a sandboxed Deno runtime.

```rust
use pctx_code_mode::config::ToolDisclosure;

let output = code_mode.execute_typescript(typescript_code, ToolDisclosure::default(), registry).await?;

if output.success {
    println!("Return value: {:?}", output.output);
    println!("Stdout: {}", output.stdout);
} else {
    eprintln!("Stderr: {}", output.stderr);
}
```

#### `execute_bash(command: &str) -> Result<ExecuteOutput>`

Executes a bash command in the virtual filesystem (used with `ToolDisclosure::Filesystem`).

```rust
let output = code_mode.execute_bash("ls -la /workspace").await?;
```

#### Accessor Methods

```rust
// Get all registered tool sets
let tool_sets: &[ToolSet] = code_mode.tool_sets();

// Get only MCP server tool sets
let server_tool_sets: &[ToolSet] = code_mode.server_tool_sets();

// Get registered server configurations
let servers: &[ServerConfig] = code_mode.servers();

// Get registered callback configurations
let callbacks: &[CallbackConfig] = code_mode.callbacks();

// Get the virtual filesystem
let vfs = code_mode.virtual_fs();
```

### PctxRegistry

Thread-safe registry that routes tool calls to Rust callbacks or MCP servers.

#### `default() -> PctxRegistry`

```rust
let registry = PctxRegistry::default();
```

#### `add_callback(id: &str, callback: CallbackFn)`

Registers a callback with a specific ID (format: `Namespace.functionName`).

```rust
registry.add_callback("DataApi.fetchData", Arc::new(|args| {
    Box::pin(async move {
        Ok(serde_json::json!({"result": "data"}))
    })
}));
```

#### `add_mcp(tool_names, cfg)`

Registers MCP tools from a server configuration.

#### `invoke(id: &str, args: Option<Value>) -> Result<Value>`

Dispatches a call by tool ID. Used internally during execution.

#### `has(id: &str) -> bool`

Checks if a tool is registered.

```rust
if registry.has("DataApi.fetchData") {
    println!("Tool is registered");
}
```

### Types

#### `CallbackConfig`

Configuration for defining callback-based tools:

```rust
pub struct CallbackConfig {
    pub name: String,
    pub namespace: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}
```

#### `Tool` and `ToolSet`

Tools represent individual functions callable from TypeScript. They are organized into ToolSets (namespaces). These are typically created internally when you call `add_callback()` or `add_server()`.

```rust
// Access registered tool sets
for tool_set in code_mode.tool_sets() {
    println!("Namespace: {}", tool_set.namespace);
    for tool in &tool_set.tools {
        println!("  - {}: {:?}", tool.fn_name, tool.description);
    }
}
```

#### `ExecuteOutput`

```rust
pub struct ExecuteOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub output: Option<serde_json::Value>,
}
```

#### `CallbackFn`

Type alias for callback functions:

```rust
pub type CallbackFn = Arc<
    dyn Fn(Option<serde_json::Value>) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
    + Send
    + Sync
>;
```

## Advanced Usage

### Adding MCP Servers

Connect to MCP (Model Context Protocol) servers to automatically register their tools:

```rust
use pctx_config::server::ServerConfig;

// Create server configuration
let server_config = ServerConfig::new_stdio("my-server", "npx", vec!["-y", "my-mcp-server"]);

// Or for HTTP-based servers
let server_config = ServerConfig::new_http("my-server", "https://api.example.com/mcp");

// Add to CodeMode (connects and registers tools)
code_mode.add_server(&server_config).await?;

// Add multiple servers in parallel with timeout
code_mode.add_servers(&[server1, server2], 30).await?;
```

### Dynamic Tool Registration

Register tools at runtime based on configuration:

```rust
use pctx_code_mode::model::CallbackConfig;

for config in tool_configs {
    code_mode.add_callback(&CallbackConfig {
        namespace: config.namespace,
        name: config.name,
        description: Some(config.description),
        input_schema: Some(config.input_schema),
        output_schema: config.output_schema,
    })?;

    // Register the corresponding callback function
    let callback_id = format!("{}.{}", config.namespace, config.name);
    registry.add_callback(&callback_id, create_callback_for_config(&config));
}
```

### Async Tool Execution

Callbacks support full async operations:

```rust
registry.add_callback("Database.query", Arc::new(|args| {
    Box::pin(async move {
        let query = args
            .and_then(|v| v.get("sql"))
            .and_then(|v| v.as_str())
            .ok_or("Missing sql parameter")?;

        // Perform async database query
        let pool = get_db_pool().await;
        let rows = sqlx::query(query)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(serde_json::to_value(rows)?)
    })
}));
```

### Error Handling

```rust
use pctx_code_mode::config::ToolDisclosure;

let output = code_mode.execute_typescript(code, ToolDisclosure::default(), registry).await?;

if !output.success {
    // Check stderr for execution errors
    if output.stderr.contains("TypeError") {
        eprintln!("Type error in generated code: {}", output.stderr);
    } else if output.stderr.contains("not found") {
        eprintln!("Tool not found: {}", output.stderr);
    } else {
        eprintln!("Execution failed: {}", output.stderr);
    }
}
```

### TypeScript Code Requirements

LLM-generated code must follow this pattern:

```typescript
async function run() {
  // Your code that calls registered tools
  const result = await Namespace.toolName({ param: value });

  // MUST return a value
  return result;
}
```

In `Sidecar` mode, use the `invoke()` function instead of namespace methods:

```typescript
async function run() {
  const result = await invoke("Namespace.toolName", { param: value });
  return result;
}
```

The code execution engine:

- Wraps your code with generated namespace implementations or an `InvokeMap` (depending on `ToolDisclosure`)
- Automatically calls `run()` and captures its return value
- Provides the return value in `ExecuteOutput.output`

## Architecture

1. **Tool Definition**: Tools are defined with JSON Schemas for inputs/outputs
2. **Disclosure Mode**: `ToolDisclosure` determines how tools are surfaced and the TypeScript code generation strategy used
3. **Code Generation**: TypeScript interface definitions are generated from schemas; `Catalog`/`Filesystem` modes emit full namespace implementations, `Sidecar` emits an `InvokeMap`
4. **Code Execution**: User code is wrapped with generated bindings and executed in Deno
5. **Call Routing**: TypeScript function calls are dispatched through `PctxRegistry` to Rust callbacks or MCP servers
6. **Result Marshaling**: JSON values are passed between TypeScript and Rust

## Sandbox Security

Code is executed in a Deno runtime with:

- Network access restricted to allowed hosts (from registered MCP servers)
- No file system access (use `execute_bash` with the virtual filesystem instead)
- No subprocess spawning
- Isolated V8 context per execution

## Examples

### Multi-Tool Workflow

```rust
use pctx_code_mode::config::ToolDisclosure;

let code = r#"
    async function run() {
        // Fetch user data
        const user = await UserApi.getUser({ id: 123 });

        // Process the data
        const processed = await DataProcessor.transform({
            input: user.data,
            format: "normalized"
        });

        // Save results
        const saved = await Storage.save({
            key: `user_${user.id}`,
            value: processed
        });

        return {
            userId: user.id,
            saved: saved.success,
            location: saved.url
        };
    }
"#;

let output = code_mode.execute_typescript(code, ToolDisclosure::Catalog, registry).await?;
```

### Error Recovery

```rust
use pctx_code_mode::config::ToolDisclosure;

let code = r#"
    async function run() {
        try {
            return await RiskyApi.operation({ id: 1 });
        } catch (error) {
            console.error("Operation failed:", error);
            // Fall back to safe default
            return await SafeApi.getDefault();
        }
    }
"#;

let output = code_mode.execute_typescript(code, ToolDisclosure::default(), registry).await?;

// Check console output
if !output.stdout.is_empty() {
    println!("Console output: {}", output.stdout);
}
```

### Parallel Execution

```rust
let code = r#"
    async function run() {
        // Execute multiple operations in parallel
        const [users, posts, comments] = await Promise.all([
            UserApi.listUsers(),
            PostApi.listPosts(),
            CommentApi.listComments()
        ]);

        return { users, posts, comments };
    }
"#;
```

## Related Crates

- `pctx_config`: Server configuration types (`ServerConfig`, `ToolDisclosure`)
- `pctx_codegen`: TypeScript code generation from JSON schemas
- `pctx_executor`: Deno runtime execution engine
- `pctx_code_execution_runtime`: Runtime environment (`PctxRegistry`, `CallbackFn`)

## License

MIT
