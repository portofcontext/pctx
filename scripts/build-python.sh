#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PCTX_PY="$REPO_ROOT/pctx-py"
SYMLINK="$PCTX_PY/src/pctx_client/descriptions/data"
SYMLINK_TARGET="../../../../crates/pctx_code_mode/descriptions"
RESOLVED=false

cleanup() {
    if [ "$RESOLVED" = true ]; then
        rm -rf "$SYMLINK"
        ln -s "$SYMLINK_TARGET" "$SYMLINK"
    fi
}

trap cleanup EXIT

if [ -L "$SYMLINK" ]; then
    REAL_TARGET="$(cd "$(dirname "$SYMLINK")" && cd "$(readlink "$SYMLINK")" && pwd)"
    rm "$SYMLINK"
    cp -r "$REAL_TARGET" "$SYMLINK"
    RESOLVED=true
fi

cd "$PCTX_PY"
uv build "$@"
