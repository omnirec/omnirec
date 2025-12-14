# Change: Add Arch User Repository (AUR) Deployment

## Why

Arch Linux users expect to install software from the AUR. Providing an official `omnirec-bin` package simplifies installation, handles dependencies correctly, and integrates with the system package manager for updates and removal.

## What Changes

- Add PKGBUILD for `omnirec-bin` package under `packaging/aur/`
- Package includes both `omnirec` main binary and `omnirec-picker` companion service
- Add `.desktop` file for application launcher integration
- Add `.install` file with post-install instructions for portal configuration
- Document AUR publishing workflow in README

## Impact

- Affected specs: None (new capability)
- Affected code: No code changes; packaging/deployment only
- New files:
  - `packaging/aur/PKGBUILD`
  - `packaging/aur/omnirec.desktop`
  - `packaging/aur/omnirec-bin.install`
  - `packaging/aur/.SRCINFO` (generated)
