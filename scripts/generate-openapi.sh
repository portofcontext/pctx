#!/bin/bash
set -e

# Generate OpenAPI documentation
echo "Generating OpenAPI documentation..."
cargo run --package pctx_session_server --bin generate-openapi
echo "✓ OpenAPI documentation generated at crates/pctx_session_server/openapi.json"
