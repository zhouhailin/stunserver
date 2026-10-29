---
name: release-version
description: Create a new release version on main branch. Updates version numbers in all Cargo.toml files, creates a git tag, and pushes to remote. Use when releasing a new version, publishing, or creating a tag. Only works on main branch with clean working tree.
---

# Release Version

Automate the release process: update versions, create tag, push to remote.

## Prerequisites

- Must be on `main` branch
- Working tree must be clean (no uncommitted changes)
- Version must follow semantic versioning (e.g., `1.2.3`, `1.2.3-beta.1`)

## Usage

Invoke with the new version number:

```bash
/release-version 1.2.3
```

Or with a pre-release version:

```bash
/release-version 1.2.3-beta.1
```

## What It Does

1. **Validates environment**
   - Confirms on `main` branch
   - Checks for uncommitted changes
   - Validates version format

2. **Updates versions**
   - Finds all `Cargo.toml` files (workspace root and crates)
   - Updates `version` field to new version
   - Commits changes with message: `chore: release version X.Y.Z`

3. **Creates and pushes tag**
   - Creates annotated tag: `vX.Y.Z`
   - Pushes tag to remote
   - Pushes `main` branch to remote

4. **Reports success**
   - Shows GitHub Release creation link
   - Suggests next steps (release notes, crates.io publish)

## Script

Execute the release script:

```bash
.qoder/skills/release-version/scripts/release.sh <version>
```

The script handles all steps automatically with validation and error handling.

## Example Workflow

```bash
# 1. Ensure on main and up to date
git checkout main
git pull origin main

# 2. Run tests and build
cargo test
cargo build --release

# 3. Release new version
/release-version 0.2.0

# 4. Create GitHub Release (use the link from script output)
# 5. Add release notes describing changes
```

## Error Handling

The script will fail and exit if:

- Not on `main` branch
- Uncommitted or staged changes exist
- Invalid version format
- Tag already exists
- Git push fails

Fix the reported issue and retry.

## Manual Recovery

If the script fails partway through:

```bash
# Check what happened
git status
git log --oneline -3
git tag -l

# If version was committed but tag not created:
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0

# If tag exists but push failed:
git push origin main
git push origin v0.2.0

# To undo and retry:
git reset --hard HEAD~1  # Remove version commit
git tag -d v0.2.0        # Remove local tag
git push origin :v0.2.0  # Remove remote tag (if pushed)
```
