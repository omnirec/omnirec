#!/usr/bin/env bash

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

IDENTITY="${1:-}"
DYLIB="src-tauri/lib/libwhisper.dylib"

if [[ -z "$IDENTITY" ]]; then
  echo "==> Skipping libwhisper.dylib signing: no macOS signing identity configured"
  exit 0
fi

if [[ ! -f "$DYLIB" ]]; then
  echo "==> Skipping libwhisper.dylib signing: $DYLIB not found"
  exit 0
fi

echo "==> Signing $DYLIB with $IDENTITY..."
codesign --force --sign "$IDENTITY" --timestamp "$DYLIB"
codesign --verify --verbose "$DYLIB"
