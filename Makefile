.PHONY: help release publish-crates docs test-cli

# Default target - show help when running just 'make'
.DEFAULT_GOAL := help

help:
	@echo "pctx dev scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  make docs           - Generate CLI, OpenAPI, and Python documentation"
	@echo "  make test-cli       - Run CLI integration tests (pctx mcp start)"
	@echo "  make release        - Interactive release script (bump version, update changelog)"
	@echo "  make publish-crates - Publish pctx_code_mode + dependencies to crates.io (runs locally)"
	@echo ""
	@echo "Python package targets live in pctx-py/Makefile (run 'make -C pctx-py help')."
	@echo ""

# Generate CLI, OAS, and Python documentation
docs:
	@./scripts/generate-cli-docs.sh
	@echo ""
	@./scripts/generate-openapi.sh
	@echo ""
	@echo "Building Python Sphinx documentation..."
	@$(MAKE) -C pctx-py docs
	@echo ""
	@echo "✓ Documentation built successfully!"

# Run CLI integration tests
test-cli:
	@./scripts/test-mcp-cli.sh

# Interactive release workflow
release:
	@./scripts/release.sh

# Publish Rust crates to crates.io
publish-crates:
	@./scripts/publish-crates.sh
