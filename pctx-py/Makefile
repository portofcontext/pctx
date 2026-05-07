.PHONY: help install test test-integration format build docs

# Default target - show help when running just 'make'
.DEFAULT_GOAL := help

help:
	@echo "pctx-py dev scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  make install          - Install dev dependencies and all extras with uv"
	@echo "  make test             - Run Python client tests"
	@echo "  make test-integration - Run Python client tests with integration testing"
	@echo "  make format           - Format and lint Python code with ruff"
	@echo "  make build            - Build Python package (resolves symlinks before build)"
	@echo "  make docs             - Build Python Sphinx documentation"
	@echo ""

# Install with dev deps and all extras
install:
	@uv sync --all-extras

# Run Python client tests
test:
	@uv run pytest tests/ -v

# Run Python client tests with integration tests (expects pctx running on localhost on the default port)
test-integration:
	@uv run pytest tests/ --integration -v

format:
	@uv run ruff format . && uv run ruff check . --fix

# Build Python package (resolves _tool_descriptions/data symlink before build, restores after)
build:
	@../scripts/build-python.sh

# Build Python Sphinx documentation
docs:
	@uv run --group docs sphinx-build -b html docs docs/_build/html
