.PHONY: help release publish-crates docs test-python test-python-integration format-python test-cli build-python

# Default target - show help when running just 'make'
.DEFAULT_GOAL := help

help:
	@echo "pctx dev scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  make docs                    - Generate CLI and Python documentation"
	@echo "  make test-python             - Run Python client tests"
	@echo "  make test-python-integration - Run Python client tests with integration testing"
	@echo "  make format-python           - Format and lint Python code with ruff"
	@echo "  make test-cli                - Run CLI integration tests (pctx mcp start)"
	@echo "  make release                 - Interactive release script (bump version, update changelog)"
	@echo "  make publish-crates          - Publish pctx_code_mode + dependencies to crates.io (runs locally)"
	@echo "  make build-python            - Build Python package (resolves symlinks before build)"
	@echo ""

# Generate CLI and Python documentation
docs:
	@./scripts/generate-cli-docs.sh
	@echo ""
	@echo "Building Python Sphinx documentation..."
	@cd pctx-py && uv run sphinx-build -b html docs docs/_build/html
	@echo ""
	@echo "✓ Documentation built successfully!"

# Run Python client tests
test-python:
	@cd pctx-py && uv run pytest tests/ -v

# Run Python client tests with integration tests (expects pctx running on localhost on the default port)
test-python-integration:
	@cd pctx-py && uv run pytest tests/ --integration -v

format-python:
	@cd pctx-py && uv run ruff format . && uv run ruff check . --fix

# Run CLI integration tests
test-cli:
	@./scripts/test-mcp-cli.sh

# Interactive release workflow
release:
	@./release.sh

# Publish Rust crates to crates.io
publish-crates:
	@./scripts/publish-crates.sh

# Build Python package (resolves _tool_descriptions/data symlink before build, restores after)
build-python:
	@./scripts/build-python.sh

