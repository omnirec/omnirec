#!/bin/bash
# Test AUR package build locally
#
# Usage:
#   ./scripts/test-aur.sh           # Auto-detect version from package.json
#   ./scripts/test-aur.sh 1.0.0     # Override with specific version

set -e

OMNIREC_DIR="$(git rev-parse --show-toplevel)"

# Get version from package.json if not provided
if [ -n "$1" ]; then
    VERSION="$1"
else
    VERSION=$(grep -o '"version": *"[^"]*"' "$OMNIREC_DIR/package.json" | cut -d'"' -f4)
    if [ -z "$VERSION" ]; then
        echo "Error: Could not detect version from package.json"
        exit 1
    fi
fi

TEST_DIR="/tmp/aur-test-$$"

echo "Testing AUR package for version $VERSION"
echo "Project directory: $OMNIREC_DIR"
echo "Test directory: $TEST_DIR"
echo ""

# Create test directory
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# Copy packaging files
cp "$OMNIREC_DIR/packaging/aur/PKGBUILD" .
cp "$OMNIREC_DIR/packaging/aur/omnirec.desktop" .
cp "$OMNIREC_DIR/packaging/aur/omnirec.install" .

# Check if local binaries exist
if [ -f "$OMNIREC_DIR/src-tauri/target/release/omnirec" ] && \
   [ -f "$OMNIREC_DIR/src-picker/target/release/omnirec-picker" ]; then
    echo "Using local build..."
    ARCHIVE_NAME="omnirec-${VERSION}-linux-x86_64"
    mkdir -p "$ARCHIVE_NAME/icons"
    
    # Copy binaries
    cp "$OMNIREC_DIR/src-tauri/target/release/omnirec" "$ARCHIVE_NAME/"
    cp "$OMNIREC_DIR/src-picker/target/release/omnirec-picker" "$ARCHIVE_NAME/"
    
    # Copy icons
    cp "$OMNIREC_DIR/src-tauri/icons/128x128.png" "$ARCHIVE_NAME/icons/"
    if [ -f "$OMNIREC_DIR/src-tauri/icons/64x64.png" ]; then
        cp "$OMNIREC_DIR/src-tauri/icons/64x64.png" "$ARCHIVE_NAME/icons/"
    else
        cp "$OMNIREC_DIR/src-tauri/icons/128x128.png" "$ARCHIVE_NAME/icons/64x64.png"
    fi
    cp "$OMNIREC_DIR/src-tauri/icons/32x32.png" "$ARCHIVE_NAME/icons/"
    
    # Copy license
    cp "$OMNIREC_DIR/LICENSE" "$ARCHIVE_NAME/"
    
    # Create archive
    tar -czvf "${ARCHIVE_NAME}.tar.gz" "$ARCHIVE_NAME"
    
    # Update PKGBUILD to use local file and correct version
    sed -i "s|^pkgver=.*|pkgver=${VERSION}|" PKGBUILD
    # Replace the entire source line to use local file (remove the renaming ::)
    sed -i "s|\"omnirec-\${pkgver}.tar.gz::\${url}/releases/download/v\${pkgver}/omnirec-\${pkgver}-linux-x86_64.tar.gz\"|\"file://${TEST_DIR}/${ARCHIVE_NAME}.tar.gz\"|" PKGBUILD
    
    echo ""
    echo "Created local archive: ${ARCHIVE_NAME}.tar.gz"
else
    echo "No local build found. Will attempt to download from GitHub release."
    echo "To use local build, first run:"
    echo "  cd src-tauri && cargo build --release"
    echo "  cd src-picker && cargo build --release"
    echo ""
    
    # Update version in PKGBUILD
    sed -i "s|^pkgver=.*|pkgver=${VERSION}|" PKGBUILD
fi

# Build package
echo ""
echo "Building package..."
makepkg -sf

# Run namcap if available
if command -v namcap &> /dev/null; then
    echo ""
    echo "Running namcap linting..."
    echo "=== PKGBUILD ===" 
    namcap PKGBUILD || true
    echo ""
    echo "=== Package ==="
    namcap omnirec-*.pkg.tar.zst || true
fi

# Generate .SRCINFO for reference
echo ""
echo "Generating .SRCINFO..."
makepkg --printsrcinfo > .SRCINFO

echo ""
echo "========================================"
echo "Package built successfully!"
echo "========================================"
echo ""
echo "Test directory: $TEST_DIR"
echo "Package file: $(ls omnirec-*.pkg.tar.zst)"
echo ""
echo "To install: sudo pacman -U $TEST_DIR/omnirec-*.pkg.tar.zst"
echo "To uninstall: sudo pacman -R omnirec"
echo ""
echo "Verification commands:"
echo "  which omnirec"
echo "  which omnirec-picker"
echo "  ls /usr/share/applications/omnirec.desktop"
echo "  omnirec"
