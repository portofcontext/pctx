# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [UNRELEASED] - YYYY-MM-DD

### Added

- `/register/tools` responses now include `warnings`: tools that registered in a degraded form, with the reason. A tool whose JSON Schema our codegen cannot express is registered with a permissive `any` signature so it stays callable; previously that only reached the server logs, invisible to a client talking to a remotely deployed session server.

### Changed

- Moved full MCP tool call / callback inputs and outputs, and `execute_typescript`/`execute_bash` code and output, from INFO to DEBUG logs to declutter traces. Added DEBUG logs describing tool result shape (structured content, JSON parse success/fallback).
- **Breaking:** `CodeMode::with_callbacks` returns `(Self, CallbackReport)` instead of `Result<Self>`, so builder-style callers see which tools failed or degraded. Per-tool isolation means the batch itself cannot fail, so the report is the only outcome.
- **Breaking:** `CodeMode::add_callback` returns `Result<Vec<String>>` — the reasons that tool's types were degraded to `any`, empty when fully typed.

### Fixed

- `-v`/`-vv` flags apply globally (previously was only applied to `pctx mcp <sub-cmd>` commands)
- `ExecuteBashOutput`'s `Display` printed `stdout` in the `# STDERR` section, so bash stderr was never shown. ([#141](https://github.com/portofcontext/pctx/issues/141))
- `/register/tools` no longer fails the whole batch when a single tool cannot be registered. Each tool is registered independently: a genuinely bad tool (name clash, unparseable schema) is skipped and returned in the response's `failed` list, and a tool our codegen cannot type degrades to an `any` signature rather than being dropped. Previously one bad tool from an upstream server — such as a recursive `$ref` in a federated schema — took down registration for the entire batch, forcing clients into ~135 sequential per-tool calls per session.
- Declared `rmcp` minimum raised from 1.2.0 to 1.8.0, the version the code actually requires. With the understated minimum, downstream consumers of the git-dep crates could resolve an older rmcp and fail to compile `pctx_session_server`.

## [v0.7.3] - 2026-07-22

### Added

- Configurable per-tool-call timeouts on `execute_typescript`, replacing a hardcoded 30s: `tool_timeout_secs` applies to every tool the execution calls, `tool_timeout_overrides` overrides individual tools by id (`namespace__name`). Both optional (default 30s), clamped to 1–600s, and available in the Python client. Bounds one call, not the whole execution.

### Changed

- Tool call timeout errors now name the tool and limit (``Tool `test_math__add` timed out after 30s``, previously `Execution timeout`).

### Fixed

- `pctx_codegen` now resolves in-document JSON pointer `$ref`s (e.g. `#/properties/filter/anyOf/0`) during type generation. Previously only `$defs`-named refs resolved, so a tool whose input schema used a pointer ref — such as a recursive query filter whose `and`/`or` groups reference the filter itself — failed type generation, and consumers fell back to typing the entire tool input as `any`. Pointer targets are now hoisted into `definitions` under a generated name and all refs to them repointed, so self-referential schemas generate proper recursive TypeScript types. Schemas already using `$defs`/`definitions` refs are unaffected.

## [v0.7.2] - 2026-07-16

### Fixed

- Stack overflow in `pctx_codegen` when generating TypeScript signatures for recursive JSON Schema `$defs` that are not object-with-properties schemas. Recursive refs now stop expanding inline and use generated type aliases, while non-recursive refs keep the existing inline behavior.
- Bound `execute_code` responses under the 16 MiB WebSocket frame limit: oversized responses (usually from a bloated trace, occasionally from a huge return value or stdout) previously exceeded the frame limit and silently failed to reach the client. The response is now shrunk before sending — dropping the trace first, then replacing the return value with a truncation marker if still too large. ***TEMPORARY FIX*** - long term suggested fix is planned in: `.plans/large-execution-payloads.md`

## [v0.7.1] - 2026-03-27

### Added

- Optional session server metadata system

### Changed

### Fixed
## [v0.7.0] - 2026-03-25

### Added

- Upstream MCP connection pooling: PCTX now maintains persistent connections to upstream MCP servers across `execute_typescript` calls, so stateful upstream servers (e.g. LSP servers, database connections) see a continuous session rather than disconnected requests.
- `pctx mcp start --stateful-http`: HTTP mode now supports stateful upstream sessions scoped to the MCP session ID. The connection pool is created on the first request and reused for the lifetime of the HTTP session, then torn down when the client sends a `DELETE`.
- `pctx mcp start --stdio`: When running as a stdio MCP server (e.g. in Claude Desktop), a single global session is used for the entire process lifetime — upstream MCP servers connect once and stay connected until `pctx` exits.
- Session server (`pctx start`): upstream connection pools are now scoped per code mode session — created on first use and cleaned up when the session is deleted.
- `ExecuteTypescriptOutput` now includes a `trace` field: a structured record of everything that happened during the execution — the type-check phase, each upstream MCP tool call (including whether the client was served from the connection pool cache), and each callback invocation — all with start/end timestamps.

### Changed

### Fixed
## [v0.6.0] - 2026-03-13

### Added

- Python `@tool` decorator now parses docstrings (Google, NumPy, reStructuredText, and Epydoc formats) to extract parameter descriptions, return value descriptions, and detailed function descriptions into tool schemas
- Make code mode config and all tools / descriptions easily configurable from python client
- `ToolDisclosure` support in python client and unified mcp with `pctx mcp start`

### Changed

- Centralized tool descriptions and workflows in root of the repo and loaded by the various `pctx` surfaces.
- Unified MCP server no longer returns structured content, most agent frameworks prioritize this, and the structured content is x2 the number of tokens than just the code-mode code

### Fixed

- Various `pctx mcp dev` rendering issues.
## [v0.5.0] - 2026-02-14

### Added

- CORS layer added to pctx session server. Add custom allowed origins via the `--allow-origin` flag
- Added more of typescript `.d.ts` files for more comprehensive type checking.

### Changed

- [#53](https://github.com/portofcontext/pctx/issues/53) Improved code generation support for tools with no input schema or all optional input schemas

### Fixed

- When removing a websocket session, graceful cancel of all pending client tool executions

- JS runtime race condition by moving the V8 mutex to be held for the entire typecheck/execute process. This previously caused a panic: `../../../../third_party/libc++/src/include/__vector/vector.h:416: libc++ Hardening assertion __n < size() failed: vector[] index out of bounds`

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

[Unreleased]: https://github.com/portofcontext/pctx/compare/v0.7.3...HEAD
[v0.7.3]: https://github.com/portofcontext/pctx/compare/v0.7.2...v0.7.3
[v0.7.2]: https://github.com/portofcontext/pctx/compare/v0.7.1...v0.7.2
[v0.7.1]: https://github.com/portofcontext/pctx/compare/v0.7.0...v0.7.1
[v0.7.0]: https://github.com/portofcontext/pctx/compare/v0.6.0...v0.7.0
[v0.6.0]: https://github.com/portofcontext/pctx/compare/v0.5.0...v0.6.0
[v0.5.0]: https://github.com/portofcontext/pctx/compare/v0.4.3...v0.5.0
[v0.4.3]: https://github.com/portofcontext/pctx/compare/v0.4.2...v0.4.3
[v0.4.2]: https://github.com/portofcontext/pctx/compare/v0.4.1...v0.4.2
[v0.4.1]: https://github.com/portofcontext/pctx/compare/v0.4.0...v0.4.1
[v0.4.0]: https://github.com/portofcontext/pctx/compare/v0.3.0...v0.4.0
[v0.3.0]: https://github.com/portofcontext/pctx/compare/v0.2.2...v0.3.0
[v0.2.2]: https://github.com/portofcontext/pctx/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/portofcontext/pctx/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/portofcontext/pctx/compare/v0.1.4...v0.2.0
[v0.1.4]: https://github.com/portofcontext/pctx/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/portofcontext/pctx/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/portofcontext/pctx/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/portofcontext/pctx/compare/v0.1.0...v0.1.1
[v0.1.0]: https://github.com/portofcontext/pctx/releases/tag/v0.1.0
