# Configuration Guide

The `pctx.json` file defines your MCP server aggregation, authentication, and runtime configuration.

## File Location

By default, `pctx` looks for `./pctx.json` in the current working directory.

Override with the `--config` flag:

```bash
pctx --config /path/to/config.json start
```

## Quick Start

Initialize a new configuration:

```bash
pctx init
```

This creates a basic `pctx.json` and prompts you to add upstream MCP servers.

## Fields

### Root Fields

| Field         | Type                  | Required | Description                                            |
| ------------- | --------------------- | -------- | ------------------------------------------------------ |
| `name`        | `string`              | Yes      | Name of your MCP server instance                       |
| `version`     | `string`              | Yes      | Version of your MCP server                             |
| `description` | `string`              | No       | Optional description of your MCP server                |
| `servers`     | `array[ServerConfig]` | Yes      | List of upstream MCP server configurations (see below) |
| `logger`      | `LoggerConfig`        | No       | Logger configuration (see below)                       |
| `telemetry`   | `TelemetryConfig`     | No       | OpenTelemetry configuration (see below)                |

### Server Configuration

Each server in the `servers` array is either an HTTP server or a stdio server.

**HTTP server fields:**

| Field  | Type         | Required | Description                                    |
| ------ | ------------ | -------- | ---------------------------------------------- |
| `name` | `string`     | Yes      | Unique identifier used as TypeScript namespace |
| `url`  | `string`     | Yes      | HTTP(S) URL of the MCP server endpoint         |
| `auth` | `AuthConfig` | No       | Authentication configuration (see below)       |

**Stdio server fields:**

| Field     | Type                | Required | Description                                                                                             |
| --------- | ------------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| `name`    | `string`            | Yes      | Unique identifier used as TypeScript namespace                                                          |
| `command` | `string`            | Yes      | Command to execute the MCP server. Can be a single command or a full command line with arguments        |
| `args`    | `array[string]`     | No       | Arguments passed to the command. If omitted and `command` contains spaces, it will be shell-parsed      |
| `env`     | `map[string]string` | No       | Environment variables for the process                                                                   |

**Examples (stdio):**

With explicit args array:
```json
{
  "name": "local_tools",
  "command": "node",
  "args": ["./dist/server.js"],
  "env": {
    "NODE_ENV": "development"
  }
}
```

With command-line string (auto-parsed):
```json
{
  "name": "memory",
  "command": "npx -y @modelcontextprotocol/server-memory"
}
```

The second format is convenient for simple commands - the full command line is automatically parsed into command and arguments.

#### Server Names as Namespaces

The `name` will be case converted to `camelCase` and used as the TypeScript namespace for accessing that server's tools:

```typescript
// Server name: "g_drive"
await gDrive.getSheet({ sheetId: "abc" });

// Server name: "slack"
await slack.sendMessage({ channel: "#general", text: "hi" });
```

**Requirements:**

- Must be unique within the configuration
- Should be a valid TypeScript identifier (alphanumeric, underscores, no spaces) to avoid clashes after case conversion
- Keep it short and descriptive

## Authentication

The `auth` field supports two types of authentication `BearerToken | Custom`:

### Bearer Token Authentication

| Field   | Type           | Required | Description                                                                                             |
| ------- | -------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| `type`  | `"bearer"`     | Yes      | Constant designating this object as a bearer token config                                               |
| `token` | `SecretString` | Yes      | Secret string value (see below for syntax) of the bearer token. `Bearer ` prefix is added automatically |

**Example:**

```json
{
  "type": "bearer",
  "token": "${env:API_TOKEN}"
}
```

This adds an `Authorization: Bearer <token>` header to all requests.

### Header Authentication

| Field     | Type                      | Required | Description                                                      |
| --------- | ------------------------- | -------- | ---------------------------------------------------------------- |
| `type`    | `"headers"`               | Yes      | Constant designating this object as a headers config             |
| `headers` | `map[string]SecretString` | Yes      | Map of header name to Secret string value (see below for syntax) |

**Example:**

```json
{
  "type": "headers",
  "headers": {
    "x-api-key": "${env:API_KEY}",
    "x-custom-header": "static-value"
  }
}
```

Use this for API key authentication or any custom header requirements.

## Logger Configuration

The optional `logger` field controls logging behavior for the pctx server MPC server. This configuration applies
to `pctx start` and MCP server modes; other commands like `pctx add` use the CLI verbosity controls (`-v/-vv/-q`).
Logs always write to stderr to keep stdout clean for JSON-RPC traffic; stdout is reserved for JSON-RPC responses.

| Field     | Type           | Required | Default     | Description                                        |
| --------- | -------------- | -------- | ----------- | -------------------------------------------------- |
| `enabled` | `boolean`      | No       | `true`      | Enable or disable logging                          |
| `level`   | `LogLevel`     | No       | `"info"`    | Minimum log level to display (see levels below)    |
| `format`  | `LoggerFormat` | No       | `"compact"` | Output format for log messages (see formats below) |
| `colors`  | `boolean`      | No       | `true`      | Enable or disable colorized output                 |

### Log Levels

Valid values for `level` (in order of increasing severity):

- `"trace"` - Most verbose, shows all logs including detailed execution traces
- `"debug"` - Detailed debugging information
- `"info"` - General informational messages (default)
- `"warn"` - Warning messages for potentially problematic situations
- `"error"` - Error messages only

### Log Formats

Valid values for `format`:

- `"compact"` - Condensed single-line format (default)
- `"pretty"` - Human-readable multi-line format with indentation
- `"json"` - Structured JSON format for log aggregation tools

### Examples

**Minimal logging (errors only):**

```json
{
  "logger": {
    "level": "error"
  }
}
```

**Debug mode with pretty formatting:**

```json
{
  "logger": {
    "enabled": true,
    "level": "debug",
    "format": "pretty",
    "colors": true
  }
}
```

**JSON logging for production (no colors):**

```json
{
  "logger": {
    "level": "info",
    "format": "json",
    "colors": false
  }
}
```

**Disable logging completely:**

```json
{
  "logger": {
    "enabled": false
  }
}
```

## Telemetry Configuration

The optional `telemetry` field enables OpenTelemetry (OTLP) integration for distributed tracing and metrics collection. This allows you to observe and monitor the behavior of your MCP server and its interactions with upstream servers.

| Field     | Type            | Required | Default | Description                       |
| --------- | --------------- | -------- | ------- | --------------------------------- |
| `traces`  | `TracesConfig`  | No       | -       | Distributed tracing configuration |
| `metrics` | `MetricsConfig` | No       | -       | Metrics collection configuration  |

### Traces Configuration

| Field       | Type                    | Required | Default | Description                              |
| ----------- | ----------------------- | -------- | ------- | ---------------------------------------- |
| `enabled`   | `boolean`               | No       | `false` | Enable or disable trace collection       |
| `exporters` | `array[ExporterConfig]` | No       | `[]`    | List of OTLP trace exporters (see below) |

### Metrics Configuration

| Field       | Type                    | Required | Default | Description                                |
| ----------- | ----------------------- | -------- | ------- | ------------------------------------------ |
| `enabled`   | `boolean`               | No       | `false` | Enable or disable metrics collection       |
| `exporters` | `array[ExporterConfig]` | No       | `[]`    | List of OTLP metrics exporters (see below) |

### Exporter Configuration

Each exporter in the `exporters` array has the following fields:

| Field      | Type               | Required | Default | Description                                            |
| ---------- | ------------------ | -------- | ------- | ------------------------------------------------------ |
| `name`     | `string`           | Yes      | -       | Identifier for this exporter                           |
| `url`      | `string`           | Yes      | -       | OTLP endpoint URL (see protocol-specific format below) |
| `protocol` | `"http" \| "grpc"` | Yes      | -       | Protocol to use for OTLP export                        |
| `timeout`  | `number`           | No       | `10000` | Request timeout in milliseconds                        |
| `auth`     | `AuthConfig`       | No       | -       | Authentication configuration (see below)               |

#### Authentication Configuration

The `auth` field supports multiple authentication methods:

**Bearer Token Authentication:**

| Field   | Type           | Required | Description                                  |
| ------- | -------------- | -------- | -------------------------------------------- |
| `type`  | `"bearer"`     | Yes      | Authentication type                          |
| `token` | `SecretString` | Yes      | Bearer token (supports secret string syntax) |

**Basic Authentication:**

| Field      | Type           | Required | Description                              |
| ---------- | -------------- | -------- | ---------------------------------------- |
| `type`     | `"basic"`      | Yes      | Authentication type                      |
| `username` | `SecretString` | Yes      | Username (supports secret string syntax) |
| `password` | `SecretString` | Yes      | Password (supports secret string syntax) |

**Custom Headers:**

| Field     | Type                      | Required | Description                                   |
| --------- | ------------------------- | -------- | --------------------------------------------- |
| `type`    | `"headers"`               | Yes      | Authentication type                           |
| `headers` | `map[string]SecretString` | Yes      | Custom headers (support secret string syntax) |

### Protocol-Specific URLs

**HTTP Protocol:**

- For traces: Include the full path including `/v1/traces`
- For metrics: Include the full path including `/v1/metrics`
- Example: `http://localhost:4318/v1/traces`

**gRPC Protocol:**

- Use the base URL without path
- Example: `http://localhost:4317`

### Examples

**Basic tracing configuration:**

```json
{
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "tempo",
          "url": "http://localhost:4318/v1/traces",
          "protocol": "http"
        }
      ]
    }
  }
}
```

**Traces and metrics with gRPC:**

```json
{
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "otlp-traces",
          "url": "http://localhost:4317",
          "protocol": "grpc"
        }
      ]
    },
    "metrics": {
      "enabled": true,
      "exporters": [
        {
          "name": "otlp-metrics",
          "url": "http://localhost:4317",
          "protocol": "grpc"
        }
      ]
    }
  }
}
```

**With bearer token authentication:**

```json
{
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "grafana-cloud",
          "url": "https://otlp-gateway.grafana.net/otlp/v1/traces",
          "protocol": "http",
          "auth": {
            "type": "bearer",
            "token": "${env:GRAFANA_CLOUD_TOKEN}"
          }
        }
      ]
    }
  }
}
```

**With basic authentication:**

```json
{
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "secure-collector",
          "url": "https://collector.example.com/v1/traces",
          "protocol": "http",
          "auth": {
            "type": "basic",
            "username": "${env:OTEL_USERNAME}",
            "password": "${env:OTEL_PASSWORD}"
          }
        }
      ]
    }
  }
}
```

**With custom headers:**

```json
{
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "custom-collector",
          "url": "https://collector.example.com/v1/traces",
          "protocol": "http",
          "auth": {
            "type": "headers",
            "headers": {
              "X-API-Key": "${env:OTEL_API_KEY}",
              "X-Custom-Header": "custom-value"
            }
          }
        }
      ]
    }
  }
}
```

**Multiple exporters:**

```json
{
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "local-tempo",
          "url": "http://localhost:4318/v1/traces",
          "protocol": "http"
        },
        {
          "name": "production-collector",
          "url": "https://otel-collector.company.com:4317",
          "protocol": "grpc",
          "timeout": 5000,
          "auth": {
            "type": "headers",
            "headers": {
              "x-api-key": "${keychain:otel-api-key}"
            }
          }
        }
      ]
    }
  }
}
```

### Getting Started with Telemetry

For a complete example with OpenTelemetry Collector, Tempo, Prometheus, and Grafana, see the [telemetry example](../examples/telemetry/README.md).

## Secret String Syntax

Both `token` and header values support a secret string syntax for secure credential management.

### Environment Variables

**Format:** `${env:VARIABLE_NAME}`

```json
{
  "token": "${env:MCP_API_TOKEN}"
}
```

The value is read from the environment variable at runtime.

**Example:**

```bash
export MCP_API_TOKEN="sk_test_123"
pctx start
```

### System Keychain

**Format:** `${keychain:KEY_NAME}`

```json
{
  "token": "${keychain:mcp-api-key}"
}
```

Reads from your OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service).

### External Commands

**Format:** `${command:shell command}`

```json
{
  "token": "${command:aws secretsmanager get-secret-value --secret-id my-token --query SecretString --output text}"
}
```

Executes the command and uses its stdout as the value (whitespace is trimmed).

**Use cases:**

- AWS Secrets Manager
- Azure Key Vault
- HashiCorp Vault
- 1Password CLI
- Pass (password store)
- Custom secret management scripts

**Examples:**

AWS Secrets Manager:

```json
{
  "type": "bearer",
  "token": "${command:aws secretsmanager get-secret-value --secret-id mcp-token --query SecretString --output text}"
}
```

1Password CLI:

```json
{
  "type": "bearer",
  "token": "${command:op read op://vault/item/field}"
}
```

### Combining Plain Text and Secrets

Secret strings support interpolation with multiple parts:

```json
{
  "headers": {
    "authorization": "ApiKey ${keychain:api-key}",
    "x-custom": "prefix-${env:SUFFIX}"
  }
}
```

### Plain Text (Not Recommended)

You can use plain text values, but this is not recommended for production:

```json
{
  "token": "sk_test_hardcoded_token"
}
```

**Warning:** Never commit credentials to version control. Use secret strings instead.

## Complete Example

```json
{
  "name": "my-ai-agent",
  "version": "1.0.0",
  "description": "MCP server aggregation for my AI agent",
  "logger": {
    "enabled": true,
    "level": "info",
    "format": "compact",
    "colors": true
  },
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "tempo",
          "url": "http://localhost:4318/v1/traces",
          "protocol": "http"
        }
      ]
    },
    "metrics": {
      "enabled": true,
      "exporters": [
        {
          "name": "prometheus",
          "url": "http://localhost:4318/v1/metrics",
          "protocol": "http"
        }
      ]
    }
  },
  "servers": [
    {
      "name": "stripe",
      "url": "https://mcp.stripe.com",
      "auth": {
        "type": "bearer",
        "token": "${env:STRIPE_MCP_KEY}"
      }
    },
    {
      "name": "gdrive",
      "url": "https://mcp.gdrive.example.com",
      "auth": {
        "type": "headers",
        "headers": {
          "x-api-key": "${keychain:gdrive-api-key}"
        }
      }
    },
    {
      "name": "internal",
      "url": "https://internal-mcp.company.com",
      "auth": {
        "type": "bearer",
        "token": "${command:vault kv get -field=token secret/mcp}"
      }
    },
    {
      "name": "public",
      "url": "https://public-mcp.example.com"
    },
    {
      "name": "memory",
      "command": "npx -y @modelcontextprotocol/server-memory"
    },
    {
      "name": "local_tools",
      "command": "node",
      "args": ["./dist/server.js"],
      "env": {
        "NODE_ENV": "production"
      }
    }
  ]
}
```

## Managing Configuration

The server configurations can be added, removed, and listed via the CLI, see [CLI Docs](./CLI.md) for details.

## Troubleshooting

### "Failed to connect" Error

**Check:**

1. URL is correct and accessible
2. Server is running
3. Network/firewall allows the connection

### "Server requires authentication" Error

The server returned 401/403. Add authentication:

```bash
pctx add my-server https://mcp.example.com \
  --bearer '${env:TOKEN}'
```

### Missing config in stdio mode

When starting with `pctx mcp start --stdio`, a missing or unreadable config file
returns a JSON-RPC error on stdout and then exits. Ensure `pctx.json` exists or
pass the correct path with `-c`.

### "Environment variable not found" Error

The specified environment variable isn't set:

```bash
# Check what's needed
cat pctx.json | grep env:

# Set the variable
export API_TOKEN="your-token"

# Or add to .env and source it
echo "API_TOKEN=your-token" >> .env
source .env
```

### "Failed to retrieve password from keychain" Error

The keychain entry doesn't exist. Create it:

```bash
# macOS
security add-generic-password -s pctx -a my-key -w "my-value"
```

Or use a different secret method.

### "Auth command failed" Error

The external command returned non-zero exit or empty output:

```bash
# Test the command directly
aws secretsmanager get-secret-value --secret-id my-token

# Check authentication for the tool
aws sts get-caller-identity
```

### Stdio Server Command Parsing

When configuring stdio servers, you have two options:

**Option 1: Shell-style command string (auto-parsed)**
```json
{
  "name": "memory",
  "command": "npx -y @modelcontextprotocol/server-memory"
}
```

The command is automatically parsed into executable and arguments using shell-style parsing.

**Option 2: Explicit command and args**
```json
{
  "name": "memory",
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-memory"]
}
```

Use explicit args when:
- Your command has complex quoting requirements
- You want to be explicit about argument boundaries
- You're programmatically generating the configuration

**Note:** If both `command` contains spaces AND `args` is provided, the `args` array takes precedence and no parsing occurs.
