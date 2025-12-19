# Change: Add automated release process with GitHub Actions

## Why

The project currently has no CI/CD pipeline. Manual releases are error-prone, time-consuming, and require access to all three target platforms (Windows, macOS, Linux). An automated release process triggered by version tags ensures consistent, reproducible builds across all platforms with proper testing and version validation.

## What Changes

- **NEW**: GitHub Actions workflow for building and releasing on tag push
- **NEW**: Multi-platform build matrix (Windows MSI, macOS DMG, Linux DEB/RPM/AppImage/tar.gz)
- **NEW**: Version validation check that compares tag version against previous release
- **NEW**: Test and lint checks that must pass before artifacts are built
- **NEW**: Draft release creation with automatically attached artifacts
- **NEW**: AUR PKGBUILD update workflow (separate from main release)
- **MODIFIED**: README badges connected to actual GitHub Actions workflow status
- **NEW**: Release-automation spec documenting the CI/CD requirements

## Impact

- **Affected specs**: None existing (new capability)
- **Affected code**:
  - `.github/workflows/` (new directory)
  - `README.md` (badge URLs)
  - `packaging/aur/PKGBUILD` (automated updates)
- **External dependencies**: GitHub Actions runners, Tauri Action
- **Risk**: Low - additive change with no impact on application code
