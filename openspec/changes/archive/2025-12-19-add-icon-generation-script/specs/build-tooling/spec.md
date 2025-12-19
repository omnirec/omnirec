# Build Tooling - Icon Generation

## ADDED Requirements

### Requirement: Icon Generation Script

The project SHALL provide a script to regenerate all application icons from the source SVG file.

#### Scenario: Generate icons from source SVG

- **WHEN** the developer runs `pnpm icons:generate`
- **THEN** the script SHALL convert `images/omnirec-icon.svg` to a high-resolution PNG
- **AND** invoke Tauri's icon generation command
- **AND** regenerate all icons in `src-tauri/icons/`

#### Scenario: Missing ImageMagick dependency

- **WHEN** the developer runs `pnpm icons:generate`
- **AND** ImageMagick is not installed
- **THEN** the script SHALL display an error message indicating ImageMagick is required
- **AND** exit with a non-zero status code

### Requirement: Icon Source File

The canonical source for the application icon SHALL be `images/omnirec-icon.svg`.

#### Scenario: Source file format

- **WHEN** regenerating icons
- **THEN** the source file SHALL be a square SVG (512x512) with transparency support
