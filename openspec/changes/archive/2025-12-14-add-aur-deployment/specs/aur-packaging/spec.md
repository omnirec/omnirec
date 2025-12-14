## ADDED Requirements

### Requirement: AUR Package Structure

The `omnirec-bin` AUR package SHALL provide a valid PKGBUILD that installs pre-built binaries for Arch Linux.

#### Scenario: Package contains required binaries

- **WHEN** the package is installed
- **THEN** `/usr/bin/omnirec` exists and is executable
- **AND** `/usr/bin/omnirec-picker` exists and is executable

#### Scenario: Package includes desktop entry

- **WHEN** the package is installed
- **THEN** `/usr/share/applications/omnirec.desktop` exists
- **AND** the application appears in desktop environment menus

#### Scenario: Package includes application icon

- **WHEN** the package is installed
- **THEN** application icons are installed to `/usr/share/icons/hicolor/`
- **AND** icons are available in standard sizes (at minimum 128x128)

### Requirement: AUR Package Dependencies

The package SHALL declare all required runtime dependencies.

#### Scenario: Runtime dependencies are declared

- **WHEN** viewing the PKGBUILD depends array
- **THEN** it includes `webkit2gtk-4.1` (Tauri runtime)
- **AND** it includes `ffmpeg` (video encoding)
- **AND** it includes `pipewire` (video capture)
- **AND** it includes `xdg-desktop-portal` (screen sharing portal)
- **AND** it includes `xdg-desktop-portal-hyprland` (Hyprland portal backend)

#### Scenario: Optional dependencies for other compositors

- **WHEN** viewing the PKGBUILD optdepends array
- **THEN** it MAY include portal backends for other Wayland compositors

### Requirement: AUR Package Metadata

The package SHALL include correct metadata for AUR listing.

#### Scenario: Package metadata is complete

- **WHEN** viewing the PKGBUILD
- **THEN** `pkgname` is `omnirec-bin`
- **AND** `pkgdesc` describes the application purpose
- **AND** `url` points to the project repository
- **AND** `license` matches the project license (MIT)
- **AND** `arch` includes `x86_64`

### Requirement: Post-Install Configuration Guidance

The package SHALL inform users of required post-install configuration steps.

#### Scenario: Install hook displays configuration instructions

- **WHEN** the package installation completes
- **THEN** a message is displayed explaining xdg-desktop-portal configuration
- **AND** the message references the README for detailed setup steps

### Requirement: AUR Publishing Readiness

The packaging files SHALL support publishing to the AUR.

#### Scenario: SRCINFO can be generated

- **WHEN** running `makepkg --printsrcinfo` in the packaging directory
- **THEN** valid `.SRCINFO` content is produced

#### Scenario: Package builds without network access during build phase

- **WHEN** `makepkg` runs the build() function
- **THEN** no network access is required (sources downloaded in source array)
