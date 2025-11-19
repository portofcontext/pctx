# SDK Configuration Design

## Overview

The `pctx_lib` crate now uses a simplified `SdkConfig` for language bindings, separate from the CLI's `Config` which includes additional concerns like logging and telemetry.

## Why Two Configs?

### `SdkConfig` (for Python/JS/TS bindings)
- **Purpose**: Clean, minimal configuration for embedding PCTX in applications
- **Users**: Python/JavaScript developers using PCTX as a library
- **Scope**: Only MCP server connections and network controls
- **Location**: `crates/pctx_lib/src/config.rs`

### `CliConfig` (for CLI application)
- **Purpose**: Full configuration including operational concerns
- **Users**: CLI users running `pctx` as a standalone MCP server
- **Scope**: Servers, logging, telemetry, and other operational settings
- **Location**: `crates/pctx_config/src/lib.rs`

## SdkConfig Structure

```rust
pub struct SdkConfig {
    /// Upstream MCP server configurations
    pub servers: Vec<ServerConfig>,
}
```

**Network Access**: Allowed hosts are automatically derived from server URLs via the `allowed_hosts()` method. There's no need to specify them separately.

## What's Excluded from SdkConfig

The following are **not** exposed in `SdkConfig` because they're CLI or server-specific:

- ❌ **Name/Version/Description** - Not relevant for SDK usage (these are server metadata)
- ❌ **Logger configuration** - SDK users control logging through their own frameworks
- ❌ **Telemetry configuration** - SDK users handle metrics in their own systems
- ❌ **File path tracking** - Not relevant when used as a library
- ❌ **Allowed hosts** - Automatically derived from server URLs
- ❌ **CLI-specific defaults** - Each SDK has its own idiomatic defaults

## Benefits

1. **Cleaner API**: SDK users only see fields relevant to their use case
2. **Better Documentation**: Type definitions don't include irrelevant fields
3. **Language Idioms**: Python/JS SDKs can use their own conventions
4. **Flexibility**: Can evolve SDK and CLI configs independently

## Conversion

The library provides automatic conversion between the two:

```rust
// CLI Config -> SDK Config
let sdk_config: SdkConfig = cli_config.into();

// SDK Config -> CLI Config (fills in defaults for missing fields)
let cli_config: CliConfig = sdk_config.into();
```

## Example Configs

### Python SDK Config
```python
pctx = Pctx(
    servers=[
        {"name": "banking", "url": "http://localhost:3000"},
    ]
)
# Allowed hosts automatically set to: ["localhost:3000"]
```

### TypeScript SDK Config
```typescript
const pctx = new Pctx({
  servers: [
    { name: 'banking', url: 'http://localhost:3000' }
  ]
});
// Allowed hosts automatically set to: ["localhost:3000"]
```

### JSON Config File
```json
{
  "servers": [
    {
      "name": "banking",
      "url": "http://localhost:3000"
    }
  ]
}
```
Note: Allowed hosts are derived from server URLs automatically.

## Implementation Notes

- The CLI (`pctx` crate) maintains its `CliConfig` for operational needs
- The library (`pctx_lib`) exposes `SdkConfig` as the primary config type
- `PctxClient::new()` takes an `SdkConfig`
- `PctxClient::from_config()` loads from JSON and creates an `SdkConfig`
- The CLI wrapper converts its config to `SdkConfig` before creating the client
