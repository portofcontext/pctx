# PCTX Code Mode

A TypeScript code execution engine that enables AI agents to dynamically call tools through generated code. Code Mode converts tool schemas (like MCP tools) into TypeScript interfaces, executes LLM-generated code in a sandboxed Deno runtime, and bridges function calls back to your Rust callbacks.

## Quick Start

```rust
use pctx_code_mode::{CodeMode, CallbackRegistry};
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
    let registry = CallbackRegistry::default();
    registry.add(&callback.id(), Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("World");
            Ok(serde_json::json!({ "message": format!("Hello, {name}!") }))
        })
    }))?;

    // 4. Execute LLM-generated TypeScript code
    let code = r#"
        async function run() {
            const result = await Greeter.greet({ name: "Alice" });
            return result;
        }
    "#;

    let output = code_mode.execute(code, Some(registry)).await?;

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

The [`CodeMode`] struct is the main execution engine. It provides:

**Builder methods** (chainable):

- `with_server()` / `with_servers()` - Add MCP servers
- `with_callback()` / `with_callbacks()` - Add callback tools

**Registration methods** (mutable):

- `add_server()` / `add_servers()` - Add MCP servers
- `add_callback()` / `add_callbacks()` - Add callback tools
- `add_tool_set()` - Add a pre-built ToolSet directly

**Accessor methods**:

- `tool_sets()` - Get registered ToolSets
- `servers()` - Get registered server configurations
- `callbacks()` - Get registered callback configurations

**Execution methods**:

- `list_functions()` - List all available functions with minimal interfaces
- `get_function_details()` - Get full typed interfaces for specific functions
- `execute()` - Execute TypeScript code in the sandbox

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

### 2. Tools and ToolSets

[`Tool`]s represent individual functions callable from TypeScript.
They are organized into [`ToolSet`]s (namespaces). Tools can be:

- **MCP tools**: Loaded from MCP servers via `add_server()`
- **Callback tools**: Defined via `CallbackConfig` and `add_callback()`

### 3. Callbacks

[`CallbackFn`] are Rust async functions that execute when TypeScript code calls callback tools.
Register them in a [`CallbackRegistry`] and pass it to `execute()`.

```rust
use pctx_code_mode::{CallbackRegistry, CallbackFn};
use std::sync::Arc;

let registry = CallbackRegistry::default();

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
registry.add("DataApi.fetchData", callback)?;
```

### 4. Code Execution

Execute LLM-generated TypeScript code that calls your registered tools.

```rust
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

let output = code_mode.execute(code, Some(registry)).await?;

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

#### `execute(code: &str, callbacks: Option<CallbackRegistry>) -> Result<ExecuteOutput>`

Executes TypeScript code in a sandboxed Deno runtime.

```rust
let output = code_mode.execute(typescript_code, Some(callback_registry)).await?;

if output.success {
    println!("Return value: {:?}", output.output);
    println!("Stdout: {}", output.stdout);
} else {
    eprintln!("Stderr: {}", output.stderr);
}
```

#### Accessor Methods

```rust
// Get registered tool sets
let tool_sets: &[ToolSet] = code_mode.tool_sets();

// Get registered server configurations
let servers: &[ServerConfig] = code_mode.servers();

// Get registered callback configurations
let callbacks: &[CallbackConfig] = code_mode.callbacks();
```

### CallbackRegistry

Thread-safe registry for managing callback functions.

#### `default() -> CallbackRegistry`

```rust
let registry = CallbackRegistry::default();
```

#### `add(id: &str, callback: CallbackFn) -> Result<()>`

Registers a callback with a specific ID (format: `Namespace.functionName`).

```rust
registry.add("DataApi.fetchData", Arc::new(|args| {
    Box::pin(async move {
        // Your implementation
        Ok(serde_json::json!({"result": "data"}))
    })
}))?;
```

#### `has(id: &str) -> bool`

Checks if a callback is registered.

```rust
if registry.has("DataApi.fetchData") {
    println!("Callback is registered");
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

```rust
use pctx_code_mode::model::CallbackConfig;
use serde_json::json;

let config = CallbackConfig {
    namespace: "MyNamespace".to_string(),
    name: "myFunction".to_string(),
    description: Some("Does something useful".to_string()),
    input_schema: Some(json!({
        "type": "object",
        "properties": { "id": { "type": "integer" } },
        "required": ["id"]
    })),
    output_schema: None,
};
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
    registry.add(&callback_id, create_callback_for_config(&config))?;
}
```

### Async Tool Execution

Callbacks support full async operations:

```rust
registry.add("Database.query", Arc::new(|args| {
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
}))?;
```

### Error Handling

```rust
let output = code_mode.execute(code, Some(registry)).await?;

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

The code execution engine:

- Wraps your code with namespace implementations
- Automatically calls `run()` and captures its return value
- Provides the return value in `ExecuteOutput.output`

## Architecture

1. **Tool Definition**: Tools are defined with JSON Schemas for inputs/outputs
2. **Code Generation**: TypeScript interface definitions are generated from schemas
3. **Code Execution**: User code is wrapped with namespace implementations and executed in Deno
4. **Callback Routing**: Function calls in TypeScript are routed to Rust callbacks or MCP servers
5. **Result Marshaling**: JSON values are passed between TypeScript and Rust

## Sandbox Security

Code is executed in a Deno runtime with:

- Network access restricted to allowed hosts (from registered MCP servers)
- No file system access
- No subprocess spawning
- Isolated V8 context per execution

```rust
// Add servers
code_mode.add_server(&server_config).await?;
```

## Examples

### Multi-Tool Workflow

```rust
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

let output = code_mode.execute(code, Some(registry)).await?;
```

### Error Recovery

```rust
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

let output = code_mode.execute(code, Some(registry)).await?;

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

- `pctx_config`: Server configuration types (`ServerConfig`)
- `pctx_codegen`: TypeScript code generation from JSON schemas
- `pctx_executor`: Deno runtime execution engine
- `pctx_code_execution_runtime`: Runtime environment and callback system

## License

MIT
