# Changelog

All notable changes to the `pctx-client` Python package will be documented in
this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this package adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

For changes to the underlying Rust crates and CLI, see the
[root CHANGELOG](../CHANGELOG.md).

## [v0.4.2] - 2026-07-17

### Added

- `Pctx(headers=...)`: an arbitrary `dict[str, str]` of headers applied to every
  HTTP request and the WebSocket connection request. Use this to authenticate
  against deployments that expect custom headers (e.g. a
  `{"authorization": "Bearer <token>"}` for GCP IAM-protected services).

### Removed

- **Breaking**: the deprecated `Pctx(api_key=...)` parameter and the
  `x-pctx-api-key` header it set. The system that validated this key is no
  longer maintained. Pass credentials via `headers=` instead — e.g.
  `Pctx(api_key="k")` → `Pctx(headers={"x-pctx-api-key": "k"})`.

## [v0.4.1] - 2026-06-08

### Added
- `py.typed` for mypy

## [v0.4.0] - 2026-05-08

### Added

- `BaseTool.input_schema` now accepts a JSON Schema dict in addition to a
  Pydantic `BaseModel` class. Dict-form schemas are validated via `jsonschema`
  (Draft 2020-12); pydantic-form schemas continue to go through
  `model_validate`. This lets integrators wrap tools defined externally
  (MCP, OpenAPI, hand-written) without round-tripping through a synthetic
  pydantic model.
- `BaseTool.output_schema` now accepts a JSON Schema dict in addition to
  Python types / typing constructs. The `TypeAdapter` path is preserved for
  rich Python output validation (`datetime`, `BaseModel` subclasses, etc.).
- `@tool` decorator: `input_schema=` and `output_schema=` keyword arguments to
  override signature inference. Useful when wrapping a function whose
  signature doesn't match the desired tool schema.
- `jsonschema>=4.26.0` added as a dependency.

### Changed

- **Breaking**: `@tool` decorator's `name` is now keyword-only.
  `@tool("custom_name")` → `@tool(name="custom_name")`. The first positional
  argument is reserved for the decorated callable so users can pass
  `input_schema=...` / `output_schema=...` without also supplying a name.
- Dict-form schemas are validated against the JSON Schema metaschema and
  compiled into a cached validator at tool construction. Malformed dict
  schemas now raise `jsonschema.SchemaError` at definition time instead of on
  first call.
- WebSocket client now surfaces `jsonschema.ValidationError` failures as
  `INVALID_PARAMS` (alongside the existing `pydantic.ValidationError` path).

## [v0.3.2] - 2026-04-06

### Changed

- Documentation dependency group split out for ReadTheDocs builds.
- Internal version bump.

## [v0.3.1] - 2026-03-25

### Added

- `p.claude_agent_sdk_tools()` returns PCTX code mode tools as Claude Agent SDK tools, optional dependency requires `pctx[claude]` extra.

## [v0.3.0] - 2026-03-12

### Added

- `@tool` decorator now parses docstrings (Google, NumPy, reStructuredText,
  and Epydoc formats) to extract parameter descriptions, return value
  descriptions, and detailed function descriptions into tool schemas.
- Code mode config and all tools / descriptions easily configurable from
  the Python client.
- `ToolDisclosure` support in the Python client and unified MCP via
  `pctx mcp start`.

### Changed

- Improved code generation support for tools with no input schema or all
  optional input schemas.

## [v0.2.0] - 2026-01-12

### Added

- Optional `search_functions` to allow the LLM to search for tools by
  name/description before deciding which tool to call.

## [v0.1.0] - 2025-12-16

### Added

- Initial release of the `pctx-client` Python package.
- `@tool` decorator and `AsyncTool` / `Tool` base classes for registering
  and interacting with the pctx session server.
- Convertors to export CodeMode tools to popular agent frameworks
  (LangChain, CrewAI, OpenAI Agents, Pydantic AI).

[v0.4.2]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.4.1...pctx-py-v0.4.2
[v0.4.1]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.4.0...pctx-py-v0.4.1
[v0.4.0]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.3.1...pctx-py-v0.4.0
[v0.3.2]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.3.1...pctx-py-v0.3.2
[v0.3.1]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.3.0...pctx-py-v0.3.1
[v0.3.0]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.3.0b1...pctx-py-v0.3.0
[v0.3.0b1]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.2.0...pctx-py-v0.3.0b1
[v0.2.0]: https://github.com/portofcontext/pctx/compare/pctx-py-v0.1.0...pctx-py-v0.2.0
[v0.1.0]: https://github.com/portofcontext/pctx/releases/tag/pctx-py-v0.1.0
