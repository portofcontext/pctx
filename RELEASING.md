# Release Process

## Prerequisites

- Write access to the repository
- Ability to trigger GitHub Actions workflows
- Access to publish to crates.io

## Release Workflow

pctx uses [cargo-dist](https://github.com/axodotdev/cargo-dist) for automated releases. The workflow is triggered manually via GitHub Actions workflow dispatch.

### Step-by-Step Process

#### 1. Prepare the Release

Update the version number in the package manifest:

```bash
# Edit crates/pctx/Cargo.toml and update the version field
# Example: version = "0.2.0"
```

#### 2. Update the Changelog

Edit [CHANGELOG.md](CHANGELOG.md) following [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
## [0.2.0] - 2025-11-10

### Added
- New feature description

### Changed
- Changed feature description

### Fixed
- Bug fix description
```

Update the comparison links at the bottom:

```markdown
[0.2.0]: https://github.com/portofcontext/pctx/compare/v0.1.0...v0.2.0
```

#### 3. Commit and Push Changes

```bash
git add crates/pctx/Cargo.toml CHANGELOG.md Cargo.lock
git commit -m "chore: prepare release v0.2.0"
git push origin main
```

#### 4. Trigger the Release

1. Go to [GitHub Actions](https://github.com/portofcontext/pctx/actions/workflows/release.yml)
2. Click "Run workflow"
3. Enter the tag name (e.g., `v0.2.0`)
4. Click "Run workflow"

The workflow will:
- Build binaries for all supported platforms (Linux, macOS, Windows)
- Create installers (shell, PowerShell, Homebrew, npm)
- Generate checksums and attestations for supply chain security
- Create a GitHub Release with all artifacts
- Publish Homebrew formula to the tap repository

#### 5. Verify the Release

Once the workflow completes (~15-20 minutes):

1. Check the [Releases page](https://github.com/portofcontext/pctx/releases)
2. Verify all platform binaries are present
3. Test the installation methods:

```bash
# Shell installer (Linux/macOS)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/portofcontext/pctx/releases/latest/download/pctx-installer.sh | sh

# PowerShell installer (Windows)
powershell -c "irm https://github.com/portofcontext/pctx/releases/latest/download/pctx-installer.ps1 | iex"

# Homebrew (macOS/Linux)
brew install portofcontext/tap/pctx

# npm (all platforms)
npm install -g @portofcontext/pctx
```


# COMING SOON
<!-- #### 6. Publish to crates.io

If publishing to crates.io:

```bash
cargo publish -p pctx
``` -->

## Release Types

### Stable Releases

Version format: `v1.0.0`, `v1.2.3`

- Fully tested and production-ready
- Published to all distribution channels
- Creates a standard GitHub Release

### Pre-releases

Version format: `v1.0.0-alpha.1`, `v1.0.0-beta.2`, `v1.0.0-rc.1`

- For testing and early access
- Marked as "pre-release" on GitHub
- May not be published to all channels

## Distribution Channels

### GitHub Releases

All releases are published to GitHub Releases with:
- Pre-built binaries for all platforms
- Source archives
- SHA256 checksums
- GitHub Attestations for supply chain security
- Auto-generated release notes from CHANGELOG.md

### Installers

#### Shell Script (Linux/macOS)

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/portofcontext/pctx/releases/latest/download/pctx-installer.sh | sh
```

Features:
- Detects platform automatically
- Verifies checksums
- Installs to `~/.local/bin`

#### PowerShell (Windows)

```powershell
powershell -c "irm https://github.com/portofcontext/pctx/releases/latest/download/pctx-installer.ps1 | iex"
```

#### Homebrew (macOS/Linux)

```bash
brew tap portofcontext/tap
brew install pctx
```

Auto-updated when releases are published.

#### npm (all platforms)

```bash
npm install -g @portofcontext/pctx
```

Provides pre-built native binaries for Node.js users.

## Platform Support

pctx is built for the following platforms:

- **Linux**: x86_64 (glibc and musl), aarch64 (glibc and musl)
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)
- **Windows**: x86_64 (MSVC)

## Best Practices

1. **Always update CHANGELOG.md** before releasing
2. **Test locally** with `dist plan` and `dist build`
3. **Use semantic versioning** (MAJOR.MINOR.PATCH)
4. **Tag after merging to main** to ensure clean builds
5. **Monitor the release workflow** to catch issues early

## Getting Help

- [cargo-dist documentation](https://opensource.axo.dev/cargo-dist/)
- [GitHub Discussions](https://github.com/portofcontext/pctx/discussions)
- [Issue tracker](https://github.com/portofcontext/pctx/issues)

