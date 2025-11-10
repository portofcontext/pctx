# Contributing to pctx

Thank you for your interest in contributing to `pctx`! This document provides guidelines and instructions for contributing.

## Code of Conduct

This project follows open source best practices. Please be respectful and constructive in all interactions.

## Getting Started

### Prerequisites

- Rust

### Setting Up Your Development Environment

1. Fork the repository on GitHub
2. Clone your fork:

```bash
git clone https://github.com/YOUR-USERNAME/pctx.git
cd pctx
```

3. Build the project:

```bash
cargo build
```

4. Run tests:

```bash
cargo test
```

5. Run the CLI:

```bash
cargo run -- --help
```

## Development Workflow

### Branching Strategy

- `main` - stable branch, releases are cut from here
- `feature/*` - new features
- `fix/*` - bug fixes
- `docs/*` - documentation updates

### Making Changes

1. Create a new branch from `main`:

```bash
git checkout -b feature/your-feature-name
```

2. Make your changes, following the code style guidelines
3. Add tests for new functionality
4. Ensure all tests pass:

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

5. Commit your changes with clear, descriptive messages:

```bash
git commit -m "feat: add new feature description"
```

### Commit Message Format

We follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `test:` - Test additions or updates
- `refactor:` - Code refactoring
- `perf:` - Performance improvements
- `chore:` - Maintenance tasks

Examples:
```
feat: add support for custom authentication providers
fix: resolve race condition in MCP client connection
docs: update installation instructions for Windows
test: add integration tests for code mode execution
```

### Pull Requests

1. Push your branch to your fork:

```bash
git push origin feature/your-feature-name
```

2. Open a pull request against the `main` branch
3. Fill out the pull request template with:
   - Description of changes
   - Related issue numbers
   - Testing performed
   - Breaking changes (if any)

4. Wait for review and address any feedback
5. Once approved, a maintainer will merge your PR

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p pctx

# Run tests with console output
cargo test -- --nocapture
```
## Code Style

### Rust Style Guidelines

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting (enforced in CI)
- Use `cargo clippy` for linting (enforced in CI)
- Write clear, self-documenting code
- Add doc comments for any public APIs

### Documentation

- Add doc comments (`///`) for all public items
- Include examples in doc comments where helpful
- Keep documentation up to date with code changes

Example:
```rust
/// Connects to an MCP server at the specified URL.
///
/// # Arguments
///
/// * `url` - The URL of the MCP server
/// * `auth` - Authentication credentials
///
/// # Returns
///
/// Returns a `Result` containing the connected client or an error.
///
/// # Examples
///
/// ```
/// let client = connect_mcp_server("https://example.com", auth)?;
/// ```
pub fn connect_mcp_server(url: &str, auth: Auth) -> Result<Client> {
    // Implementation
}
```

## Project Structure

## Areas for Contribution

### Good First Issues

Look for issues labeled `good first issue` for beginner-friendly tasks:
- Documentation improvements
- Test coverage enhancements
- Bug fixes with clear reproduction steps

### Feature Requests

- Check existing issues before creating new ones
- Discuss major features in an issue before implementing
- Break large features into smaller, reviewable PRs

### Bug Reports

When reporting bugs, please include:
- Clear description of the issue
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Rust version, etc.)
- Error messages or logs

## Release Process

Releases are managed by maintainers. See [RELEASING.md](RELEASING.md) for the full process.

As a contributor:
- Update CHANGELOG.md with your changes under "Unreleased"
- Note any breaking changes in your PR description
- Follow semantic versioning principles

## Community

- GitHub Discussions: For questions and general discussion
- GitHub Issues: For bug reports and feature requests
- Pull Requests: For code contributions

## Legal

By contributing to pctx, you agree that your contributions will be licensed under the MIT License.

All contributions must be your original work or properly attributed if based on other sources.

## Questions?

If you have questions about contributing, feel free to:
- Open a GitHub Discussion
- Comment on an existing issue
- Ask in your pull request

## Recognition

Contributors are recognized in:
- Git commit history
- Release notes (for significant contributions)
- GitHub's contributor graph (coming soon)