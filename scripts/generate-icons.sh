#!/usr/bin/env bash
#
# Generate all application icons from the source SVG.
# Requires: ImageMagick (magick command)
#
# Usage: ./scripts/generate-icons.sh
#        pnpm icons:generate

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE_SVG="$PROJECT_ROOT/images/omnirec-icon.svg"
TEMP_PNG="$PROJECT_ROOT/images/omnirec-icon-temp.png"
ICON_OUTPUT_DIR="$PROJECT_ROOT/src-tauri/icons"
ASSET_ICON="$PROJECT_ROOT/src/assets/omnirec-icon.png"

# Check for ImageMagick
if ! command -v magick &> /dev/null; then
    echo "Error: ImageMagick is not installed or 'magick' command not found."
    echo "Please install ImageMagick: https://imagemagick.org/script/download.php"
    exit 1
fi

# Check source SVG exists
if [[ ! -f "$SOURCE_SVG" ]]; then
    echo "Error: Source SVG not found at $SOURCE_SVG"
    exit 1
fi

echo "Converting SVG to PNG..."
magick -background none "$SOURCE_SVG" -resize 1024x1024 "$TEMP_PNG"

echo "Generating Tauri icons..."
cd "$PROJECT_ROOT"
pnpm tauri icon "$TEMP_PNG" --output "$ICON_OUTPUT_DIR"

echo "Generating frontend asset icon..."
magick -background none "$SOURCE_SVG" -resize 512x512 "$ASSET_ICON"

# =============================================================================
# Generate symbolic/monochrome tray icons for Linux
# =============================================================================
# These are single-color icons that work with GNOME/GTK system trays.
# They should be white silhouettes with transparency - the system will
# colorize them appropriately for light/dark themes.
#
# Standard sizes for system tray icons:
# - 16x16: Very small displays
# - 22x22: Standard GNOME panel size
# - 24x24: Alternative panel size
# - 32x32: HiDPI panels (1.5x scale)
# - 48x48: HiDPI panels (2x scale)
# =============================================================================

TRAY_ICON_DIR="$ICON_OUTPUT_DIR/tray"
mkdir -p "$TRAY_ICON_DIR"

echo "Generating symbolic tray icons..."

# Generate white monochrome version from the source
# We convert all non-transparent pixels to white (#FFFFFF)
for size in 16 22 24 32 48; do
    echo "  - ${size}x${size} symbolic icon"
    
    # Create monochrome white version
    # 1. Resize the source SVG
    # 2. Extract alpha channel
    # 3. Use alpha as mask for solid white
    magick -background none "$SOURCE_SVG" -resize "${size}x${size}" \
        -alpha extract \
        -background white -alpha shape \
        "$TRAY_ICON_DIR/omnirec-symbolic-${size}.png"
done

# Also create a "recording" variant (red dot) for when recording is active
echo "Generating recording state tray icons..."

for size in 16 22 24 32 48; do
    echo "  - ${size}x${size} recording icon"
    
    # Create a red version for recording state
    magick -background none "$SOURCE_SVG" -resize "${size}x${size}" \
        -alpha extract \
        -background "#E53935" -alpha shape \
        "$TRAY_ICON_DIR/omnirec-recording-${size}.png"
done

# Create copies with standard names for the most common size (22x22)
# (Using copies instead of symlinks for better bundle compatibility)
echo "Creating standard icon copies..."
cp "$TRAY_ICON_DIR/omnirec-symbolic-22.png" "$TRAY_ICON_DIR/omnirec-symbolic.png"
cp "$TRAY_ICON_DIR/omnirec-recording-22.png" "$TRAY_ICON_DIR/omnirec-recording.png"

echo "Cleaning up temporary file..."
rm -f "$TEMP_PNG"

echo ""
echo "Done! Icons generated:"
echo "  - Tauri icons: $ICON_OUTPUT_DIR"
echo "  - Frontend asset: $ASSET_ICON"
echo "  - Tray icons: $TRAY_ICON_DIR"
echo ""
echo "Tray icon files:"
ls -la "$TRAY_ICON_DIR"
