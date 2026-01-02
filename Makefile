.PHONY: help release docs test-python

# Default target - show help when running just 'make'
.DEFAULT_GOAL := help

help:
	@echo "pctx dev scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  make docs                    - Generate CLI and Python documentation"
	@echo "  make test-python             - Run Python client tests"
	@echo "  make test-python-integration - Run Python client tests with integration testing"
	@echo "  make release                 - Interactive release script (bump version, update changelog)"
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

# Interactive release workflow
release:
	@./release.sh


