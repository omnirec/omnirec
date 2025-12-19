# Change: Add Icon Generation Script

## Why

Currently, regenerating product icons from the source SVG (`images/omnirec-icon.svg`) requires manual steps. A script will automate icon generation, ensuring consistency and simplifying the workflow when the icon design changes.

## What Changes

- Add a shell script (`scripts/generate-icons.sh`) that regenerates all Tauri icons from the source SVG
- Script uses ImageMagick to convert SVG to high-resolution PNG, then delegates to `pnpm tauri icon`
- Add npm script `icons:generate` for easy invocation
- Document the icon generation workflow in README

## Impact

- Affected specs: None (new tooling-only capability)
- Affected code:
  - `scripts/generate-icons.sh` (new)
  - `package.json` (add script entry)
  - `README.md` (documentation)
  - `src-tauri/icons/` (regenerated assets)
