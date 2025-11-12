# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [UNRELEASED] - YYYY-MM-DD

### Added

- `pctx add` now accepts `--headers` and `--bearer` to add authentication without interaction

### Fixed

- Catch user cancellations when adding MCP servers in `pctx init`

## [0.1.2] - 2025-12-10

### Fixed

- Synced deno runtime op stubs and JS config interfaces to match dev, supporting auth in built CLI.

## [0.1.1] - 2025-11-10

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

## [0.1.0] - 2025-11-10

### Added

- Initial public release

[Unreleased]: https://github.com/portofcontext/pctx/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/portofcontext/pctx/compare/v0.1.2
[0.1.1]: https://github.com/portofcontext/pctx/compare/v0.1.1
[0.1.0]: https://github.com/portofcontext/pctx/releases/tag/v0.1.0
