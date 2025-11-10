# PCTX Type Check

An isolated TypeScript type checking runtime powered by Deno and the official TypeScript compiler.

## Quick Start

```rust
use pctx_type_check::{type_check_async, type_check};

// Async API (preferred)
let code = r#"
    const x: number = 42;
    const y: string = "hello";
    export default x + y.length;
"#;

let result = type_check_async(code).await?;
if result.success {
    println!("✓ Type check passed!");
} else {
    for diagnostic in result.diagnostics {
        println!("{}: {}", diagnostic.severity, diagnostic.message);
    }
}

// Sync API (creates tokio runtime if needed)
let result = type_check(code)?;
```

## API Reference

### Core Functions

#### `type_check_async(code: &str) -> Result<CheckResult>`

Asynchronously type check TypeScript code. **Preferred API** for async contexts.

```rust
let result = type_check_async(code).await?;
assert!(result.success);
```

#### `type_check(code: &str) -> Result<CheckResult>`

Synchronously type check TypeScript code. Creates a tokio runtime if needed.

```rust
let result = type_check(code)?;
assert!(result.success);
```

#### `is_relevant_error(diagnostic: &Diagnostic) -> bool`

Filter diagnostics to only include errors that would cause runtime failures.

```rust
let errors: Vec<_> = result.diagnostics
    .into_iter()
    .filter(|d| is_relevant_error(d))
    .collect();
```

### Types

#### `CheckResult`

```rust
pub struct CheckResult {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
}
```

#### `Diagnostic`

```rust
pub struct Diagnostic {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub severity: String, // "error" or "warning"
    pub code: Option<u32>, // TypeScript error code
}
```

#### `TypeCheckError`

```rust
pub enum TypeCheckError {
    InternalError(String),
    ParseError(String),
}
```


## Examples

### Catching Type Errors

```rust
let code = r#"
    const x: number = "string"; // Type error!
    export default x;
"#;

let result = type_check_async(code).await?;
assert!(!result.success);
assert_eq!(result.diagnostics[0].code, Some(2322)); // Type mismatch
```


### Architecture

1. **Build Phase**: TypeScript compiler (9MB) is embedded in a V8 snapshot via `build.rs`
2. **Runtime Phase**: Each type check creates an isolated Deno runtime with the snapshot
3. **Type Checking**: Runtime executes `ts.createProgram()` and `getSemanticDiagnostics()`
4. **Cleanup**: Runtime is dropped after check, freeing all memory

### Snapshot Contents

The V8 snapshot includes:

- TypeScript 5.3.3 compiler (full semantic analysis)
- Minimal `lib.d.ts` definitions (Promise, Array, console, etc.)
- MCP SDK type definitions
- Type checking orchestration logic

### Filtered Error Codes

See `is_relevant_error()` for a full list of typescript codes that are ignored