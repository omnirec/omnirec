## Context

OmniRec is a cross-platform Tauri application supporting Windows, macOS, and Linux. Releases currently require manual building on each platform. The project uses:
- Tauri v2 for desktop packaging (MSI, DMG, DEB, RPM, AppImage)
- pnpm for frontend dependencies
- Cargo for Rust backend
- An existing AUR PKGBUILD for Arch Linux distribution

GitHub is the hosting platform, making GitHub Actions the natural choice for CI/CD.

## Goals / Non-Goals

### Goals
- Automate multi-platform builds on version tag push (e.g., `v0.2.0`)
- Validate version increment before proceeding with build
- Run all tests and lints before building artifacts
- Create draft releases with attached artifacts for manual review
- Update README badges to reflect actual build/release status
- Provide foundation for future AUR automation

### Non-Goals
- Automatic publishing to AUR (manual step for now; can be added later)
- Code signing for Windows/macOS (separate initiative)
- Automatic changelog generation
- Nightly/development builds

## Decisions

### Decision: Use Tauri Action for builds
The official `tauri-apps/tauri-action` handles cross-platform builds efficiently, manages artifact naming, and integrates well with GitHub releases. This avoids maintaining custom build scripts for each platform.

**Alternatives considered:**
- Custom build matrix with manual `pnpm tauri build`: More control but significantly more maintenance burden
- Third-party actions: Less community support and may lag behind Tauri updates

### Decision: Tag-triggered workflow only
The release workflow triggers only on version tags (`v*.*.*`). Separate workflows handle PRs and main branch commits for testing.

**Alternatives considered:**
- Manual dispatch workflow: Less convenient, risk of human error in version selection
- Branch-based releases: More complex to manage, doesn't match semantic versioning expectations

### Decision: Version validation via GitHub API
Compare the tag version against the latest GitHub release using the GitHub API. This is simpler than parsing Cargo.toml from previous commits and handles edge cases (e.g., no previous releases).

**Alternatives considered:**
- Parse git tags: Requires sorting logic for semver comparison
- Cargo.toml comparison: Requires fetching previous commit's file

### Decision: Separate CI and release workflows
- `ci.yml`: Runs on PRs and main branch pushes (lint, test, typecheck)
- `release.yml`: Runs on version tags (validation, build, release creation)

This separation keeps the release workflow focused and allows CI to run without building full artifacts.

### Decision: Draft releases by default
Create releases as drafts so maintainers can review artifacts and edit release notes before publishing. This prevents accidental releases of broken builds.

### Decision: Linux tar.gz for AUR
Build a Linux tar.gz archive containing binaries and icons that the AUR PKGBUILD can download. This matches the existing PKGBUILD source expectation.

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| GitHub Actions runner availability | Build delays | Low risk; GitHub-hosted runners are reliable |
| Tauri Action version incompatibility | Build failures | Pin action versions, test before updating |
| macOS/Windows signing requirements | Unsigned warnings | Document as known limitation; address separately |
| Large artifact storage | GitHub storage limits | Use LFS for large files; artifacts auto-expire |

## Workflow Architecture

```
push tag v*.*.*
    │
    ├── release.yml
    │   ├── validate-version (compare with latest release)
    │   ├── test-and-lint (full test suite)
    │   └── build-and-release (matrix: windows, macos, linux)
    │       └── create draft release + attach artifacts
    │
push PR/main
    │
    └── ci.yml
        ├── lint (cargo clippy, pnpm lint)
        ├── test (cargo test)
        └── typecheck (tsc --noEmit)
```

## Artifact Matrix

| Platform | Format | Tauri Target | Notes |
|----------|--------|--------------|-------|
| Windows | MSI | `msi` | Primary Windows installer |
| Windows | NSIS | `nsis` | Alternative installer |
| macOS | DMG | `dmg` | Disk image for drag-and-drop install |
| macOS | App bundle | `app` | Raw .app for manual install |
| Linux | DEB | `deb` | Debian/Ubuntu package |
| Linux | RPM | `rpm` | Fedora/RHEL package |
| Linux | AppImage | `appimage` | Universal Linux format |
| Linux | tar.gz | Custom | Binary archive for AUR |

## Badge Configuration

Update README badges to use actual workflow URLs:
- Build: `https://img.shields.io/github/actions/workflow/status/omnirec/omnirec/ci.yml?branch=main&label=build`
- Tests: `https://img.shields.io/github/actions/workflow/status/omnirec/omnirec/ci.yml?branch=main&label=tests`
- Release: `https://img.shields.io/github/v/release/omnirec/omnirec`
- License: `https://img.shields.io/github/license/omnirec/omnirec`
- AUR: `https://img.shields.io/aur/version/omnirec-bin`

## Open Questions

1. Should we add code signing for Windows/macOS in this iteration? (Recommendation: No, defer to separate proposal)
2. Should the AUR PKGBUILD update be automated or remain manual? (Recommendation: Manual for now, can automate later with SSH deploy key)
3. Should we support ARM64 macOS builds? (Recommendation: Yes, Tauri Action supports universal binaries)
