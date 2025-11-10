#!/bin/sh
set -e

# Fetch the latest release version from GitHub
LATEST_RELEASE=$(curl -s https://api.github.com/repos/portofcontext/pctx/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    echo "Error: Could not fetch latest release version"
    exit 1
fi

echo "Installing pctx ${LATEST_RELEASE}..."

# Download and run the installer for the latest version
curl --proto '=https' --tlsv1.2 -LsSf "https://github.com/portofcontext/pctx/releases/download/${LATEST_RELEASE}/pctx-installer.sh" | sh
