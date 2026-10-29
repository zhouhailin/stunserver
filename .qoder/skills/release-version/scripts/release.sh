#!/usr/bin/env bash
# Release version script
# Updates version in Cargo.toml files, creates tag, and pushes to remote
# Must be run on main branch with no uncommitted changes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print error and exit
error() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

# Print success message
success() {
    echo -e "${GREEN}✓ $1${NC}"
}

# Print info message
info() {
    echo -e "${YELLOW}→ $1${NC}"
}

# Check if on main branch
check_branch() {
    local branch
    branch=$(git branch --show-current)
    if [ "$branch" != "main" ]; then
        error "Must be on 'main' branch to release (currently on '$branch')"
    fi
    success "On main branch"
}

# Check for uncommitted changes
check_clean_working_tree() {
    if ! git diff --quiet HEAD; then
        error "Uncommitted changes detected. Please commit or stash changes first."
    fi
    if ! git diff --cached --quiet; then
        error "Staged changes detected. Please commit them first."
    fi
    success "Working tree is clean"
}

# Validate version format (semver)
validate_version() {
    local version="$1"
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?(\+[a-zA-Z0-9.]+)?$ ]]; then
        error "Invalid version format: '$version'. Use semantic versioning (e.g., 1.2.3, 1.2.3-beta.1)"
    fi
    success "Version format is valid: $version"
}

# Get current version from root Cargo.toml
get_current_version() {
    local version
    version=$(grep -m 1 '^version\s*=' Cargo.toml | sed 's/version\s*=\s*"\(.*\)"/\1/')
    if [ -z "$version" ]; then
        error "Could not find version in Cargo.toml"
    fi
    echo "$version"
}

# Update version in all Cargo.toml files
update_versions() {
    local new_version="$1"
    local files_updated=0
    
    info "Updating version to $new_version in Cargo.toml files..."
    
    # Find all Cargo.toml files (workspace root and all crates)
    while IFS= read -r -d '' file; do
        if grep -q '^version\s*=' "$file"; then
            # Update version field
            sed -i "s/^version\s*=\s*\".*\"/version = \"$new_version\"/" "$file"
            success "Updated $file"
            ((files_updated++))
        fi
    done < <(find . -name "Cargo.toml" -not -path "./target/*" -print0)
    
    if [ "$files_updated" -eq 0 ]; then
        error "No Cargo.toml files with version field found"
    fi
    
    success "Updated $files_updated file(s)"
}

# Commit version changes
commit_version() {
    local version="$1"
    info "Committing version update..."
    git add -A
    git commit -m "chore: release version $version"
    success "Committed version update"
}

# Create and push tag
create_tag() {
    local version="$1"
    local tag="v$version"
    
    # Check if tag already exists
    if git rev-parse "$tag" >/dev/null 2>&1; then
        error "Tag '$tag' already exists. Delete it first with: git tag -d $tag"
    fi
    
    info "Creating tag $tag..."
    git tag -a "$tag" -m "Release $tag"
    success "Created tag $tag"
    
    info "Pushing tag to remote..."
    git push origin "$tag"
    success "Pushed tag to remote"
}

# Push main branch
push_main() {
    info "Pushing main branch to remote..."
    git push origin main
    success "Pushed main to remote"
}

# Print summary
print_summary() {
    local version="$1"
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Release v$version completed successfully!${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Create a GitHub Release: https://github.com/$(git remote get-url origin | sed 's/.*github.com[:/]\(.*\)\.git/\1')/releases/new?tag=v$version"
    echo "  2. Add release notes describing changes"
    echo "  3. Publish to crates.io if needed: cargo publish"
    echo ""
}

# Main execution
main() {
    local new_version="${1:-}"
    
    if [ -z "$new_version" ]; then
        echo "Usage: $0 <version>"
        echo "Example: $0 1.2.3"
        exit 1
    fi
    
    echo -e "${GREEN}Starting release process for version $new_version...${NC}"
    echo ""
    
    # Pre-flight checks
    check_branch
    check_clean_working_tree
    validate_version "$new_version"
    
    local current_version
    current_version=$(get_current_version)
    info "Current version: $current_version"
    info "New version: $new_version"
    echo ""
    
    # Update versions
    update_versions "$new_version"
    echo ""
    
    # Commit
    commit_version "$new_version"
    echo ""
    
    # Create and push tag
    create_tag "$new_version"
    echo ""
    
    # Push main
    push_main
    echo ""
    
    # Summary
    print_summary "$new_version"
}

main "$@"
