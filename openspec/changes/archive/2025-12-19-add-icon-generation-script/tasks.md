# Tasks

## 1. Implementation

- [x] 1.1 Create `scripts/generate-icons.sh` script
  - Convert SVG to 1024x1024 PNG using ImageMagick (`magick` command)
  - Invoke `pnpm tauri icon` with the generated PNG
  - Clean up temporary PNG file after generation
  - Add error handling for missing dependencies (ImageMagick)

- [x] 1.2 Add npm script to `package.json`
  - Add `"icons:generate": "bash scripts/generate-icons.sh"` script entry

- [x] 1.3 Update README.md
  - Document ImageMagick as a development dependency
  - Add instructions for regenerating icons

## 2. Validation

- [x] 2.1 Test script execution
  - Verify script runs successfully on the development environment
  - Confirm all icon files in `src-tauri/icons/` are regenerated
  - Verify iOS and Android icons are generated correctly
