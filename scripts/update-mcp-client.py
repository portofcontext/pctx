#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""
Update mcp-client.min.mjs from the CDN.

Usage:
    uv run scripts/update-mcp-client.py
"""

import urllib.request
from pathlib import Path


CDN_URL = "https://cdn.jsdelivr.net/npm/@portofcontext/mcp-client@latest/dist/index.mjs"


def download_file(url: str, dest: Path) -> None:
    """Download a file from a URL to a destination path."""
    print(f"Downloading {url}...")
    with urllib.request.urlopen(url) as response:
        content = response.read()
        with open(dest, "wb") as f:
            f.write(content)
    print(f"✓ Downloaded {len(content)} bytes")


def main():
    # Determine paths
    script_dir = Path(__file__).parent
    repo_root = script_dir.parent
    dest_file = repo_root / "crates" / "sdk_runner" / "js" / "mcp-client.min.mjs"

    # Ensure parent directory exists
    dest_file.parent.mkdir(parents=True, exist_ok=True)

    # Download the file
    download_file(CDN_URL, dest_file)

    print(f"\n✓ Successfully updated mcp-client.min.mjs!")
    print(f"  Location: {dest_file}")


if __name__ == "__main__":
    main()
