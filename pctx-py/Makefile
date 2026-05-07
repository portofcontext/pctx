.PHONY: help install test test-integration fmt lint typecheck check build docs

# Default target - show help when running just 'make'
.DEFAULT_GOAL := help

help:
	@echo "pctx-py dev scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  make install          - Install all dependency groups and all extras with uv"
	@echo "  make test             - Run Python client tests"
	@echo "  make test-integration - Run Python client tests with integration testing"
	@echo "  make fmt           - Format and auto-fix lints with ruff"
	@echo "  make lint             - Lint Python code with ruff"
	@echo "  make typecheck        - Type-check Python code with ty"
	@echo "  make check            - Run lint, typecheck, and tests"
	@echo "  make build            - Build Python package (resolves symlinks before build)"
	@echo "  make docs             - Build Python Sphinx documentation"
	@echo ""

# Install with all dependency groups (dev, docs) and all extras
install:
	@uv sync --all-extras --all-groups

# Run Python client tests
test:
	@uv run pytest tests/ -v

# Run Python client tests with integration tests (expects pctx running on localhost on the default port)
test-integration:
	@uv run pytest tests/ --integration -v

fmt:
	@uv run ruff format . && uv run ruff check . --fix

# Lint with ruff (no auto-fix)
lint:
	@uv run ruff check .

# Type-check with ty
typecheck:
	@uv run ty check src

# Run lint, typecheck, and tests
check: lint typecheck test

# Build Python package (resolves _tool_descriptions/data symlink before build, restores after)
build:
	@../scripts/build-python.sh

# Build Python Sphinx documentation
docs:
	@uv run --group docs sphinx-build -b html docs docs/_build/html
