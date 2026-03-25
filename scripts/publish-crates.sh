#!/usr/bin/env bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() { echo -e "${BLUE}ℹ ${NC}$1"; }
print_success() { echo -e "${GREEN}✓${NC} $1"; }
print_warning() { echo -e "${YELLOW}⚠${NC} $1"; }
print_error() { echo -e "${RED}✗${NC} $1"; }

# Check if cargo-smart-release is installed
if ! command -v cargo-smart-release &> /dev/null; then
    print_error "cargo-smart-release is not installed"
    echo ""
    echo "Install it with:"
    echo "  cargo install cargo-smart-release"
    exit 1
fi

# Default to pctx_code_mode if no argument provided
CRATE_NAME="${1:-pctx_code_mode}"

# Verify crate exists
if [ ! -d "crates/$CRATE_NAME" ]; then
    print_error "Crate 'crates/$CRATE_NAME' does not exist"
    exit 1
fi

print_info "Publishing crate: $CRATE_NAME (with dependencies)"
print_info "cargo-smart-release will automatically publish all required dependencies"
echo ""

# Get bump type
print_info "Select version bump type:"
echo "  1) auto   - Determine from git history (default)"
echo "  2) patch  - Bug fixes"
echo "  3) minor  - New features"
echo "  4) major  - Breaking changes"
echo "  5) keep   - Keep current version"
echo ""

read -r -p "Enter choice [1-5] (default: 1): " choice
choice=${choice:-1}

case "$choice" in
    1) BUMP_TYPE="auto" ;;
    2) BUMP_TYPE="patch" ;;
    3) BUMP_TYPE="minor" ;;
    4) BUMP_TYPE="major" ;;
    5) BUMP_TYPE="keep" ;;
    *)
        print_error "Invalid choice"
        exit 1
        ;;
esac

echo ""
print_info "=== DRY RUN ==="
print_info "Running: cargo smart-release $CRATE_NAME --bump $BUMP_TYPE --no-changelog-preview --allow-fully-generated-changelogs --update-crates-index"
print_info "This will show all crates that need to be published (including dependencies)"
echo ""

cargo smart-release "$CRATE_NAME" --bump "$BUMP_TYPE" --no-changelog-preview --allow-fully-generated-changelogs --update-crates-index

echo ""
print_warning "This was a dry run. Review the output above."
print_warning "cargo-smart-release will publish dependencies in the correct order"
echo ""
read -r -p "Proceed with actual release? [y/N]: " confirm

if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    print_warning "Release cancelled"
    exit 0
fi

echo ""
print_info "=== EXECUTING RELEASE ==="
print_info "Auto-generated changelogs will be created from commit history"
echo ""

cargo smart-release "$CRATE_NAME" --bump "$BUMP_TYPE" --no-changelog-preview --allow-fully-generated-changelogs --execute

echo ""
print_success "Release completed successfully!"
echo ""
print_info "The release has:"
echo "  - Updated versions in Cargo.toml (for $CRATE_NAME and dependencies)"
echo "  - Created git commits and tags"
echo "  - Published to crates.io (in dependency order)"
echo "  - Pushed commits and tags to git"
