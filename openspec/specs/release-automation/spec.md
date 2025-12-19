# release-automation Specification

## Purpose
TBD - created by archiving change add-github-release-automation. Update Purpose after archive.
## Requirements
### Requirement: CI Workflow

The system SHALL run continuous integration checks on pull requests and main branch pushes.

#### Scenario: PR triggers CI workflow
- **WHEN** a pull request is opened or updated
- **THEN** the CI workflow runs lint, test, and typecheck jobs

#### Scenario: Main branch push triggers CI workflow
- **WHEN** code is pushed to the main branch
- **THEN** the CI workflow runs lint, test, and typecheck jobs

#### Scenario: CI runs on all platforms
- **WHEN** the CI workflow runs
- **THEN** tests execute on Windows, macOS, and Linux runners

### Requirement: Release Workflow Trigger

The system SHALL trigger the release workflow when a version tag is pushed.

#### Scenario: Version tag triggers release
- **WHEN** a tag matching pattern `v*.*.*` is pushed
- **THEN** the release workflow is triggered

#### Scenario: Non-version tag does not trigger release
- **WHEN** a tag not matching `v*.*.*` is pushed
- **THEN** the release workflow is NOT triggered

### Requirement: Version Validation

The system SHALL validate that the release version is greater than the previous release.

#### Scenario: Version is incremented
- **WHEN** the tag version is greater than the latest GitHub release version
- **THEN** the workflow proceeds to build artifacts

#### Scenario: Version is not incremented
- **WHEN** the tag version is less than or equal to the latest GitHub release version
- **THEN** the workflow fails with a version validation error

#### Scenario: No previous release exists
- **WHEN** no previous GitHub release exists
- **THEN** the workflow proceeds to build artifacts (first release)

### Requirement: Pre-Release Checks

The system SHALL run all tests and lints before building release artifacts.

#### Scenario: Tests pass before build
- **WHEN** the release workflow runs
- **THEN** cargo tests, clippy checks, and TypeScript type checks run before artifact building

#### Scenario: Tests fail blocks release
- **WHEN** any test or lint check fails
- **THEN** the workflow stops and does not build artifacts

### Requirement: Multi-Platform Build

The system SHALL build release artifacts for all supported platforms.

#### Scenario: Windows artifact build
- **WHEN** the release workflow builds artifacts
- **THEN** Windows MSI and NSIS installers are created

#### Scenario: macOS artifact build
- **WHEN** the release workflow builds artifacts
- **THEN** macOS DMG and app bundle are created as universal binaries (ARM64 and x64)

#### Scenario: Linux artifact build
- **WHEN** the release workflow builds artifacts
- **THEN** Linux DEB, RPM, and AppImage packages are created

#### Scenario: Linux tar.gz archive for AUR
- **WHEN** the release workflow builds artifacts
- **THEN** a tar.gz archive containing binaries, icons, and LICENSE is created for AUR distribution

### Requirement: Draft Release Creation

The system SHALL create a draft GitHub release with attached artifacts.

#### Scenario: Draft release created
- **WHEN** all artifacts are built successfully
- **THEN** a draft GitHub release is created with all artifacts attached

#### Scenario: Release title matches tag
- **WHEN** a draft release is created
- **THEN** the release title includes the version from the tag

### Requirement: README Badge Accuracy

The README SHALL display badges that reflect actual build and release status.

#### Scenario: Build badge reflects CI status
- **WHEN** a user views the README
- **THEN** the build badge shows the status of the CI workflow on main branch

#### Scenario: Release badge reflects latest release
- **WHEN** a user views the README
- **THEN** the release badge shows the latest published release version

#### Scenario: AUR badge reflects AUR package version
- **WHEN** a user views the README
- **THEN** the AUR badge shows the current version in the AUR repository

