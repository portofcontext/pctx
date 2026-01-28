# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [UNRELEASED] - YYYY-MM-DD

### Added

### Changed

### Fixed

## [v0.4.3] - 2026-01-27

### Added

### Changed

### Fixed

- Callback configurations with empty input schemas causing panic -> now empty input schemas will be handled as `any`, and fully support of optional schemas will be released later.

## [v0.4.2] - 2026-01-19

### Added

### Changed

### Fixed

- OpenTelemetry distributed tracing support via W3C `traceparent` header in both MCP and session servers

## [v0.4.1] - 2026-01-12

### Added

- Improve instrumentation
- Optional `search_functions` in the python client to allow the LLM to search
  for tools by name/description before deciding which tool to call.

### Changed

### Fixed

## [v0.4.0] - 2025-12-31

### Added

- Stdio MCP server support for upstreams via `pctx.json` (`command`, `args`, `env`).
- `pctx mcp add` now supports stdio MCP servers via `--command`, `--arg`, and `--env` flags.
- `pctx mcp start --stdio` to serve the MCP interface over stdio.
- Logger configuration now supports optional `file` field to write logs to a file.

### Changed

- `pctx mcp add` now accepts either a URL (for HTTP servers) or `--command` (for stdio servers), making it a unified interface for adding all types of MCP servers.
- Logger output behavior is now mode-aware to ensure stdio compatibility:
  - `--stdio` mode without `logger.file`: logging is automatically disabled to keep stdout/stderr clean for JSON-RPC communication
  - `--stdio` mode with `logger.file`: logs write to the specified file
  - HTTP mode: logs write to stdout (default behavior)

### Fixed

- Improved error handling for stdio config and MCP initialization failures.

## [v0.3.0] - 2025-12-16

### Added

- `pctx_session_server` crate implements CodeMode sessions using HTTP endpoints for session management and websockets for code execution with callbacks to user-defined tools.
- `pctx_core` crate created as the primary code mode library via the `CodeMode` struct. With support for MCP servers and callback functions.
- `pctx_executor`/`pctx_code_execution_runtime`/`pctx_type_check_runtime` supports callbacks to arbitrary rust callables
- `pctx-client` (Python) package with `@tool` decorator and `AsyncTool`/`Tool` base class for registering/interacting with the pctx session server. Users can export the CodeMode tools to popular agent frameworks like langchain.

### Changed

- **Breaking Change**: `pctx start` now starts the pctx session server, all previous commands have been migrated to `pctx mcp <subcommand>`.
- `codegen` create extended to include generic `Tool` and `ToolSet` structs and all code generation functions migrated to be methods of these structs.

### Fixed

- `[additionalProperty: string]: ...` not included when `additionalProperties: false` in schema.
- Comments above `[additionalProperty: string]: ...` now correctly document the expected additional property types.

## [v0.2.2] - 2025-12-07

### Added

- windows cross-compile support through cargo-dist

## [v0.2.1] - 2025-11-25

### Added

- All tools return define `outputSchema` and return `structuredOutput` alongside the text content.

### Fixed

- `pctx add`
  - Prefer env var over keychain auth in interactive upstream mcp adding
- `pctx dev`
  - Better error state reporting (e.g. invalid config, port already in use)
  - Scroll out of bounds for tool details panel

### Changed

- Auth type `custom`, changed to `headers` to be more descriptive. `custom` retained as an alias for backwards compatibility

## [v0.2.0] - 2025-11-19

### Added

- `pctx dev` command with Terminal UI to explore CodeMode interface and track requests when running PCTX locally
- `logger` configuration in `pctx.json` (`pctx_config::logger::LoggerConfig`) that supports configuring stdout logging level, format, and colorization
- `telemetry` configuration in `pctx.json` (`pctx_config::telemetry::TelemetryConfig`) that supports enabling exporters for traces and metrics
  - `examples/telemetry` example docker compose setup for Tempo/Prometheus/Grafana to try out these new configs

## [v0.1.4] - 2025-11-14

### Added

- nasa mcp server example with scripts for running/deploying pctx

### Fixed

- ts code ignore syncing
- remove slow intel mac runner

## [v0.1.3] - 2025-11-13

### Added

- `pctx add` now accepts `--header` and `--bearer` to add authentication without interaction
- `pctx.json` config now accepts version which gets returned as the MCP's version in the `initialize` MCP response
- add typescript type check runtime capabilities including more typical string/array utils
- tool descriptions updated for consistent behavior

### Fixed

- Catch user cancellations when adding MCP servers in `pctx init`

## [v0.1.2] - 2025-11-12

### Fixed

- Synced deno runtime op stubs and JS config interfaces to match dev, supporting auth in built CLI.

## [v0.1.1] - 2025-11-10

### Added

- Initial release of pctx
- Code mode interface for AI agent code execution
- Upstream MCP server aggregation through a single interface
- Secure authentication system (environment variables, keychain, arbitrary commands)
- 2 Isolated Deno sandboxes: one for type checking and one for secure code execution
- MCP server to agents functionality
- Authentication and route management

### Security

- Code runs in isolated Deno sandbox with network host restrictions
- No filesystem, environment, or system access beyond allowed hosts
- MCP clients are authenticated, credentials hidden from LLMs an Deno env

## [v0.1.0] - 2025-11-10

### Added

- Initial public release

[Unreleased]: https://github.com/portofcontext/pctx/compare/v0.3.0...HEAD
[v0.2.1]: https://github.com/portofcontext/pctx/compare/v0.3.0
[v0.2.1]: https://github.com/portofcontext/pctx/compare/v0.2.2
[v0.2.1]: https://github.com/portofcontext/pctx/compare/v0.2.1
[v0.2.0]: https://github.com/portofcontext/pctx/compare/v0.2.0
[v0.1.4]: https://github.com/portofcontext/pctx/compare/v0.1.4
[v0.1.3]: https://github.com/portofcontext/pctx/compare/v0.1.3
[v0.1.2]: https://github.com/portofcontext/pctx/compare/v0.1.2
[v0.1.1]: https://github.com/portofcontext/pctx/compare/v0.1.1
[v0.1.0]: https://github.com/portofcontext/pctx/releases/tag/v0.1.0
