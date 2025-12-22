#!/usr/bin/env bash
#
# Set the version number across all project configuration files.
#
# Usage: ./scripts/set-version.sh <version>
#        pnpm version:set 1.0.0
#
# Example: ./scripts/set-version.sh 1.2.3
#
# This updates the version in:
#   - package.json
#   - src-tauri/Cargo.toml
#   - src-tauri/tauri.conf.json
#   - packaging/aur/PKGBUILD

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Validate version argument
if [[ $# -ne 1 ]]; then
    echo -e "${RED}Error: Version argument required${NC}"
    echo "Usage: $0 <version>"
    echo "Example: $0 1.2.3"
    exit 1
fi

VERSION="$1"

# Validate version format (semver: major.minor.patch with optional pre-release)
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo -e "${RED}Error: Invalid version format '${VERSION}'${NC}"
    echo "Version must be in semver format: X.Y.Z or X.Y.Z-prerelease"
    echo "Examples: 1.0.0, 2.1.3, 1.0.0-beta.1"
    exit 1
fi

echo -e "${YELLOW}Setting version to ${VERSION}...${NC}"
echo ""

# Track success/failure
ERRORS=0

# Function to update JSON file version
update_json_version() {
    local file="$1"
    local relative_path="${file#$PROJECT_ROOT/}"
    
    if [[ ! -f "$file" ]]; then
        echo -e "  ${RED}✗${NC} $relative_path (file not found)"
        ((ERRORS++))
        return
    fi
    
    # Use a temp file for atomic update
    local tmp_file=$(mktemp)
    
    # Update version field in JSON (handles both "version": "x.y.z" formats)
    if sed -E 's/("version"[[:space:]]*:[[:space:]]*")([^"]+)(")/\1'"$VERSION"'\3/' "$file" > "$tmp_file"; then
        mv "$tmp_file" "$file"
        echo -e "  ${GREEN}✓${NC} $relative_path"
    else
        rm -f "$tmp_file"
        echo -e "  ${RED}✗${NC} $relative_path (sed failed)"
        ((ERRORS++))
    fi
}

# Function to update TOML file version (in [package] section)
update_toml_version() {
    local file="$1"
    local relative_path="${file#$PROJECT_ROOT/}"
    
    if [[ ! -f "$file" ]]; then
        echo -e "  ${RED}✗${NC} $relative_path (file not found)"
        ((ERRORS++))
        return
    fi
    
    local tmp_file=$(mktemp)
    
    # Update version in [package] section (first occurrence of version = "...")
    if sed -E '0,/^version[[:space:]]*=[[:space:]]*"[^"]+"/s/^version[[:space:]]*=[[:space:]]*"[^"]+"/version = "'"$VERSION"'"/' "$file" > "$tmp_file"; then
        mv "$tmp_file" "$file"
        echo -e "  ${GREEN}✓${NC} $relative_path"
    else
        rm -f "$tmp_file"
        echo -e "  ${RED}✗${NC} $relative_path (sed failed)"
        ((ERRORS++))
    fi
}

# Function to update PKGBUILD version
update_pkgbuild_version() {
    local file="$1"
    local relative_path="${file#$PROJECT_ROOT/}"
    
    if [[ ! -f "$file" ]]; then
        echo -e "  ${RED}✗${NC} $relative_path (file not found)"
        ((ERRORS++))
        return
    fi
    
    local tmp_file=$(mktemp)
    
    # Update pkgver=X.Y.Z
    if sed -E 's/^pkgver=.+$/pkgver='"$VERSION"'/' "$file" > "$tmp_file"; then
        mv "$tmp_file" "$file"
        echo -e "  ${GREEN}✓${NC} $relative_path"
    else
        rm -f "$tmp_file"
        echo -e "  ${RED}✗${NC} $relative_path (sed failed)"
        ((ERRORS++))
    fi
}

# Update all version files
echo "Updating version files:"

update_json_version "$PROJECT_ROOT/package.json"
update_json_version "$PROJECT_ROOT/src-tauri/tauri.conf.json"
update_toml_version "$PROJECT_ROOT/src-tauri/Cargo.toml"
update_pkgbuild_version "$PROJECT_ROOT/packaging/aur/PKGBUILD"

echo ""

# Summary
if [[ $ERRORS -eq 0 ]]; then
    echo -e "${GREEN}Successfully set version to ${VERSION} in all files!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Review changes: git diff"
    echo "  2. Commit: git commit -am \"chore: bump version to ${VERSION}\""
    echo "  3. Tag: git tag v${VERSION}"
    echo "  4. Push: git push && git push --tags"
else
    echo -e "${RED}Completed with ${ERRORS} error(s)${NC}"
    exit 1
fi
