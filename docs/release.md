# Release Process

This document describes the release process for STUN Server.

## Versioning

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** version: Incompatible API changes
- **MINOR** version: Backward-compatible functionality additions
- **PATCH** version: Backward-compatible bug fixes

Example: `v1.2.3`

## Release Workflow

### 1. Prepare the Release

```bash
# Ensure you're on main branch
git checkout main
git pull upstream main

# Update version in Cargo.toml files
# (all workspace members should have the same version)
cargo set-version 1.2.3

# Or manually edit each Cargo.toml:
# - /Cargo.toml (workspace.package.version)
# - /stun-protocol/Cargo.toml
# - /stunserver/Cargo.toml
# - /stunclient/Cargo.toml
# - /stunbench/Cargo.toml

# Update CHANGELOG.md (if maintained)
# Commit version bump
git add .
git commit -m "chore: bump version to 1.2.3"
```

### 2. Run Pre-Release Checks

```bash
# Run all tests
cargo test --all

# Check code coverage (must be >= 95%)
cargo tarpaulin --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Build release binaries
cargo build --release

# Test binaries
./target/release/stunserver --version
./target/release/stunclient --version
./target/release/stunbench --version
```

### 3. Create Release Tag

```bash
# Create annotated tag
git tag -a v1.2.3 -m "Release v1.2.3"

# Push tag to upstream
git push upstream v1.2.3
```

### 4. Automated Release

The GitHub Actions workflow (`.github/workflows/release.yml`) automatically:

1. Detects the new tag
2. Builds binaries for all platforms:
   - Linux x86_64 (GNU)
   - Linux aarch64 (GNU)
   - macOS x86_64
   - macOS aarch64 (Apple Silicon)
   - Windows x86_64 (MSVC)
3. Creates a GitHub Release
4. Uploads binary artifacts

### 5. Verify Release

After the workflow completes:

1. Go to [GitHub Releases](https://github.com/yourusername/stunserver/releases)
2. Verify all artifacts are uploaded
3. Test download links
4. Check release notes are generated correctly

### 6. Announce Release

- Update project documentation if needed
- Announce in project channels (if applicable)
- Update package registries (if applicable)

## Release Artifacts

Each release produces:

### Binaries

- `stunserver-linux-x86_64.tar.gz`
- `stunserver-linux-aarch64.tar.gz`
- `stunserver-macos-x86_64.tar.gz`
- `stunserver-macos-aarch64.tar.gz`
- `stunserver-windows-x86_64.zip`

Each archive contains:
- `stunserver` (or `stunserver.exe`)
- `stunclient` (or `stunclient.exe`)
- `stunbench` (or `stunbench.exe`)

### Source Code

GitHub automatically generates source archives:
- `Source code (tar.gz)`
- `Source code (zip)`

## Hotfix Releases

For critical bugs:

```bash
# Create hotfix branch from tag
git checkout -b hotfix/1.2.4 v1.2.3

# Make fixes
git commit -m "fix: resolve critical issue"

# Bump version
cargo set-version 1.2.4
git commit -m "chore: bump version to 1.2.4"

# Merge to main
git checkout main
git merge hotfix/1.2.4

# Tag and push
git tag -a v1.2.4 -m "Hotfix v1.2.4"
git push upstream main
git push upstream v1.2.4
```

## Rollback Procedure

If a release has critical issues:

1. **Do not delete the tag** (preserves history)
2. Create a new patch release with fixes
3. Mark the problematic release as "pre-release" on GitHub
4. Add a note to the release description

## Release Checklist

Before creating a release tag:

- [ ] All tests pass (`cargo test --all`)
- [ ] Code coverage >= 95% (`cargo tarpaulin --all`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt --all -- --check`)
- [ ] Release builds successfully (`cargo build --release`)
- [ ] Binaries tested manually
- [ ] Version numbers updated in all Cargo.toml files
- [ ] CHANGELOG updated (if maintained)
- [ ] Documentation updated
- [ ] No uncommitted changes

## Troubleshooting

### Build Failures

If the release build fails:

1. Check the GitHub Actions logs
2. Reproduce locally with the same target
3. Fix issues and create a new tag

### Missing Artifacts

If artifacts are missing:

1. Check the workflow completed successfully
2. Verify the tag format matches `v*` pattern
3. Check artifact upload step in logs

### Incorrect Version

If the version is wrong:

1. Delete the tag: `git tag -d v1.2.3`
2. Delete remote tag: `git push upstream :refs/tags/v1.2.3`
3. Fix version in Cargo.toml
4. Create new tag

## Security

### Signing Releases

For additional security, consider signing releases:

```bash
# Sign tag with GPG
git tag -s v1.2.3 -m "Release v1.2.3"
```

### Verify Signatures

Users can verify:

```bash
git tag -v v1.2.3
```

## Support

For release-related issues:

- Open a GitHub issue
- Check existing issues and discussions
- Review GitHub Actions logs
