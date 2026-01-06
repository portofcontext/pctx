.PHONY: help release docs test-python bench bench-direct bench-compare bench-install

# Default target - show help when running just 'make'
.DEFAULT_GOAL := help

help:
	@echo "pctx dev scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  make docs                    - Generate CLI and Python documentation"
	@echo "  make test-python             - Run Python client tests"
	@echo "  make test-python-integration - Run Python client tests with integration testing"
	@echo "  make bench                   - Run MCP-Bench WITH pctx (MODEL=model TASKS=n TASK=task_id VERBOSE=1)"
	@echo "  make bench-direct            - Run MCP-Bench WITHOUT pctx (MODEL=model TASKS=n TASK=task_id VERBOSE=1)"
	@echo "  make bench-compare           - Run both modes for comparison (TASK=task_id)"
	@echo "  make bench-install           - Install MCP servers for benchmarks"
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

# Run MCP-Bench benchmarks WITH pctx unified interface
bench:
	@cd benchmarks && \
		if [ -f .env ]; then \
			export $$(cat .env | grep -v '^#' | xargs) && echo "Loaded API keys from .env"; \
		fi && \
		if [ -z "$$OPENROUTER_API_KEY" ]; then \
			echo "Error: OPENROUTER_API_KEY not set"; \
			echo "Set it in benchmarks/.env or export it"; \
			exit 1; \
		fi && \
		echo "Running benchmarks WITH pctx..." && \
		if [ -n "$(TASK)" ]; then \
			uv run run_with_pctx.py \
				--model $(or $(MODEL),deepseek/deepseek-chat) \
				--task-id $(TASK) \
				$(if $(VERBOSE),--verbose); \
		else \
			uv run run_with_pctx.py \
				--model $(or $(MODEL),deepseek/deepseek-chat) \
				$(if $(TASKS),--max-tasks $(TASKS)) \
				$(if $(VERBOSE),--verbose); \
		fi

# Run MCP-Bench benchmarks WITHOUT pctx (direct MCP server connections)
bench-direct:
	@cd benchmarks && \
		if [ -f .env ]; then \
			export $$(cat .env | grep -v '^#' | xargs) && echo "Loaded API keys from .env"; \
		fi && \
		if [ -z "$$OPENROUTER_API_KEY" ]; then \
			echo "Error: OPENROUTER_API_KEY not set"; \
			echo "Set it in benchmarks/.env or export it"; \
			exit 1; \
		fi && \
		echo "Running benchmarks WITHOUT pctx (direct mode)..." && \
		if [ -n "$(TASK)" ]; then \
			uv run run_without_pctx.py \
				--model $(or $(MODEL),deepseek/deepseek-chat) \
				--task-id $(TASK) \
				$(if $(VERBOSE),--verbose); \
		else \
			uv run run_without_pctx.py \
				--model $(or $(MODEL),deepseek/deepseek-chat) \
				$(if $(TASKS),--max-tasks $(TASKS)) \
				$(if $(VERBOSE),--verbose); \
		fi

# Run both modes for comparison
bench-compare:
	@if [ -z "$(TASK)" ]; then \
		echo "Error: TASK parameter required for comparison"; \
		echo "Usage: make bench-compare TASK=wikipedia_000"; \
		exit 1; \
	fi
	@echo "========================================"
	@echo "Running WITH pctx..."
	@echo "========================================"
	@$(MAKE) bench TASK=$(TASK) MODEL=$(or $(MODEL),deepseek/deepseek-chat)
	@echo ""
	@echo "========================================"
	@echo "Running WITHOUT pctx (direct mode)..."
	@echo "========================================"
	@$(MAKE) bench-direct TASK=$(TASK) MODEL=$(or $(MODEL),deepseek/deepseek-chat)
	@echo ""
	@echo "========================================"
	@echo "Comparison complete!"
	@echo "Check benchmarks/results/ for detailed results"
	@echo "========================================"

# Install MCP servers for benchmarks
bench-install:
	@echo "Installing MCP servers from mcp-bench..."
	@cd benchmarks/mcp-bench/mcp_servers && bash ./install.sh
	@echo "✓ MCP servers installed"


