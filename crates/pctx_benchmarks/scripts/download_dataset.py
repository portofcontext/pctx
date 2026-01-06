#!/usr/bin/env python3
"""
Download MCP-Bench dataset from the official repository.

Usage:
    python scripts/download_dataset.py
    python scripts/download_dataset.py --output custom/path
"""

import argparse
import sys
from pathlib import Path

try:
    import requests
except ImportError:
    print("Error: requests library not found")
    print("Install with: pip install requests")
    sys.exit(1)


DATASET_URLS = {
    "single": "https://raw.githubusercontent.com/Accenture/mcp-bench/main/tasks/mcpbench_tasks_single_runner_format.json",
    "multi_2server": "https://raw.githubusercontent.com/Accenture/mcp-bench/main/tasks/mcpbench_tasks_multi_2server_runner_format.json",
    "multi_3server": "https://raw.githubusercontent.com/Accenture/mcp-bench/main/tasks/mcpbench_tasks_multi_3server_runner_format.json",
}


def download_dataset(output_dir: Path, dataset_type: str = "single"):
    """Download MCP-Bench dataset to the specified directory."""

    # Create output directory if it doesn't exist
    output_dir.mkdir(parents=True, exist_ok=True)

    # Get URL for dataset type
    url = DATASET_URLS.get(dataset_type)
    if not url:
        print(f"Error: Unknown dataset type '{dataset_type}'")
        print(f"Available types: {', '.join(DATASET_URLS.keys())}")
        return False

    # Determine output filename
    filename = f"mcpbench_tasks_{dataset_type}_runner_format.json"
    output_path = output_dir / filename

    print(f"Downloading MCP-Bench dataset ({dataset_type})...")
    print(f"From: {url}")
    print(f"To: {output_path}")

    try:
        response = requests.get(url, timeout=30)
        response.raise_for_status()

        # Save to file
        with open(output_path, "w") as f:
            f.write(response.text)

        print(f"✓ Successfully downloaded {len(response.text)} bytes")
        return True

    except requests.RequestException as e:
        print(f"✗ Download failed: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(
        description="Download MCP-Bench dataset from official repository"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).parent.parent / "data",
        help="Output directory for dataset (default: data/ relative to script)",
    )
    parser.add_argument(
        "--type",
        choices=list(DATASET_URLS.keys()),
        default="single",
        help="Dataset type to download (default: single)",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Download all dataset types",
    )

    args = parser.parse_args()

    if args.all:
        # Download all dataset types
        success = True
        for dataset_type in DATASET_URLS.keys():
            if not download_dataset(args.output, dataset_type):
                success = False
            print()  # Blank line between downloads

        if success:
            print("✓ All datasets downloaded successfully")
            return 0
        else:
            print("✗ Some downloads failed")
            return 1
    else:
        # Download single dataset type
        if download_dataset(args.output, args.type):
            return 0
        else:
            return 1


if __name__ == "__main__":
    sys.exit(main())
