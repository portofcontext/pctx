#!/usr/bin/env bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_TOML="${SCRIPT_DIR}/crates/pctx/Cargo.toml"
CHANGELOG="${SCRIPT_DIR}/CHANGELOG.md"

# Function to print colored output
print_info() { echo -e "${BLUE}ℹ ${NC}$1"; }
print_success() { echo -e "${GREEN}✓${NC} $1"; }
print_warning() { echo -e "${YELLOW}⚠${NC} $1"; }
print_error() { echo -e "${RED}✗${NC} $1"; }

# Function to get current version from Cargo.toml
get_current_version() {
    grep '^version = ' "$CARGO_TOML" | head -n1 | sed 's/version = "\(.*\)"/\1/'
}

# Function to bump version
bump_version() {
    local version=$1
    local bump_type=$2

    IFS='.' read -r major minor patch <<< "$version"

    case "$bump_type" in
        major)
            echo "$((major + 1)).0.0"
            ;;
        minor)
            echo "${major}.$((minor + 1)).0"
            ;;
        patch)
            echo "${major}.${minor}.$((patch + 1))"
            ;;
        *)
            print_error "Invalid bump type: $bump_type"
            exit 1
            ;;
    esac
}

# Function to update version in Cargo.toml
update_cargo_version() {
    local new_version=$1

    # Update version in crates/pctx/Cargo.toml
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        sed -i '' "s/^version = \".*\"/version = \"$new_version\"/" "$CARGO_TOML"
    else
        # Linux
        sed -i "s/^version = \".*\"/version = \"$new_version\"/" "$CARGO_TOML"
    fi

    print_success "Updated version in Cargo.toml to $new_version"
}

# Function to extract UNRELEASED section content from CHANGELOG
extract_unreleased_content() {
    local unreleased_line=$(grep -n "^## \[UNRELEASED\]" "$CHANGELOG" | cut -d: -f1)

    if [[ -z "$unreleased_line" ]]; then
        print_error "Could not find UNRELEASED section in CHANGELOG.md"
        exit 1
    fi

    # Find the next version section
    local next_version_line=$(tail -n +$((unreleased_line + 1)) "$CHANGELOG" | grep -n "^## \[v" | head -n1 | cut -d: -f1)

    if [[ -n "$next_version_line" ]]; then
        local end_line=$((unreleased_line + next_version_line - 1))
        # Extract content between UNRELEASED and next version
        sed -n "$((unreleased_line + 1)),$((end_line))p" "$CHANGELOG"
    else
        # Extract from UNRELEASED to end of file
        tail -n +$((unreleased_line + 1)) "$CHANGELOG"
    fi
}

# Function to update changelog
update_changelog() {
    local new_version=$1
    local unreleased_content=$2
    local date=$(date +%Y-%m-%d)

    # Find the line number of "## [UNRELEASED]"
    local unreleased_line=$(grep -n "^## \[UNRELEASED\]" "$CHANGELOG" | cut -d: -f1)

    if [[ -z "$unreleased_line" ]]; then
        print_error "Could not find UNRELEASED section in CHANGELOG.md"
        exit 1
    fi

    # Find the next version section
    local next_version_line=$(tail -n +$((unreleased_line + 1)) "$CHANGELOG" | grep -n "^## \[v" | head -n1 | cut -d: -f1)

    if [[ -n "$next_version_line" ]]; then
        local end_line=$((unreleased_line + next_version_line - 1))
    else
        # If no next version, go to end of file
        local end_line=$(wc -l < "$CHANGELOG")
    fi

    # Create new changelog with:
    # 1. Content before UNRELEASED
    # 2. Fresh UNRELEASED header
    # 3. New version section with the old unreleased content
    # 4. Rest of the changelog
    {
        # Keep everything before UNRELEASED line
        head -n $((unreleased_line - 1)) "$CHANGELOG"

        # Add fresh UNRELEASED section
        echo "## [UNRELEASED] - YYYY-MM-DD"
        echo ""
        echo "### Added"
        echo ""
        echo "### Changed"
        echo ""
        echo "### Fixed"
        echo ""

        # Add new version section with the extracted content
        echo "## [v${new_version}] - ${date}"
        echo "$unreleased_content"

        # Add rest of changelog (from next version onwards)
        if [[ -n "$next_version_line" ]]; then
            tail -n +$((unreleased_line + next_version_line)) "$CHANGELOG"
        fi
    } > "${CHANGELOG}.tmp"

    mv "${CHANGELOG}.tmp" "$CHANGELOG"

    print_success "Updated CHANGELOG.md with v${new_version} release"
}

# Main script
main() {
    print_info "pctx Release Script"
    echo ""

    # Get current version
    CURRENT_VERSION=$(get_current_version)
    print_info "Current version: ${CURRENT_VERSION}"
    echo ""

    # Ask for bump type
    print_info "Select version bump type:"
    echo "  1) patch (${CURRENT_VERSION} → $(bump_version "$CURRENT_VERSION" patch))"
    echo "  2) minor (${CURRENT_VERSION} → $(bump_version "$CURRENT_VERSION" minor))"
    echo "  3) major (${CURRENT_VERSION} → $(bump_version "$CURRENT_VERSION" major))"
    echo ""

    read -r -p "Enter choice [1-3]: " choice

    case "$choice" in
        1) BUMP_TYPE="patch" ;;
        2) BUMP_TYPE="minor" ;;
        3) BUMP_TYPE="major" ;;
        *)
            print_error "Invalid choice"
            exit 1
            ;;
    esac

    NEW_VERSION=$(bump_version "$CURRENT_VERSION" "$BUMP_TYPE")
    echo ""
    print_info "New version will be: ${NEW_VERSION}"
    echo ""

    # Extract UNRELEASED section from CHANGELOG
    echo ""
    print_info "=== Extracting UNRELEASED changes from CHANGELOG.md ==="
    echo ""
    UNRELEASED_CONTENT=$(extract_unreleased_content)

    # Preview the changes
    print_info "=== Preview ==="
    echo ""
    echo "Version: ${CURRENT_VERSION} → ${NEW_VERSION}"
    echo ""
    echo "Changelog content from UNRELEASED section:"
    echo ""
    echo "$UNRELEASED_CONTENT"
    echo ""

    # Confirm
    read -r -p "Proceed with release? [y/N]: " confirm

    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        print_warning "Release cancelled"
        exit 0
    fi

    echo ""
    print_info "=== Applying Changes ==="
    echo ""

    # Update Cargo.toml
    update_cargo_version "$NEW_VERSION"

    # Update CHANGELOG.md
    update_changelog "$NEW_VERSION" "$UNRELEASED_CONTENT"

    echo ""
    print_success "Release v${NEW_VERSION} prepared successfully!"
    echo ""
    print_info "Next steps:"
    echo "  1. Review the changes in Cargo.toml and CHANGELOG.md"
    echo "  2. Run 'cargo test' to ensure everything works"
    echo "  3. Commit and push the changes"
    echo "  3. Dispatch release from GitHub Actions"
    echo "  4. If the Python SDK needs releasing, bump pyproject.toml, run make test-python (uv.lock) use the GH action manual dispatch"

}

main "$@"
