#!/bin/bash
set -e

# Generate CLI documentation using clap-markdown
echo "Generating CLI documentation..."
cargo run --bin generate-cli-docs > docs/CLI.md
echo "✓ CLI documentation generated at docs/CLI.md"
