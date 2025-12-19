## 1. GitHub Actions CI Workflow

- [x] 1.1 Create `.github/workflows/ci.yml` with lint, test, and typecheck jobs
- [x] 1.2 Configure job matrix for all three platforms (ubuntu-latest, macos-latest, windows-latest)
- [x] 1.3 Add Rust toolchain setup with caching
- [x] 1.4 Add pnpm setup with caching
- [x] 1.5 Run `cargo clippy` for Rust linting
- [x] 1.6 Run `cargo test` for Rust tests
- [x] 1.7 Run `tsc --noEmit` for TypeScript type checking
- [x] 1.8 Configure workflow to trigger on PRs and main branch pushes

## 2. GitHub Actions Release Workflow

- [x] 2.1 Create `.github/workflows/release.yml` triggered on `v*.*.*` tags
- [x] 2.2 Add version validation job comparing tag against latest GitHub release
- [x] 2.3 Fail workflow if version is not incremented
- [x] 2.4 Add test-and-lint job that runs full test suite before building
- [x] 2.5 Configure build matrix using Tauri Action for all platforms
- [x] 2.6 Build Windows MSI and NSIS installers
- [x] 2.7 Build macOS DMG and app bundle (universal binary for ARM64/x64)
- [x] 2.8 Build Linux DEB, RPM, and AppImage packages
- [x] 2.9 Create Linux tar.gz archive with binaries and icons for AUR
- [x] 2.10 Create draft GitHub release with all artifacts attached

## 3. Update tauri.conf.json Bundle Targets

- [x] 3.1 Add all bundle targets: `["msi", "nsis", "dmg", "app", "deb", "rpm", "appimage"]`

## 4. Update README Badges

- [x] 4.1 Update build badge URL to point to `ci.yml` workflow
- [x] 4.2 Update tests badge URL to point to `ci.yml` workflow
- [x] 4.3 Update release badge URL to use correct repository path
- [x] 4.4 Update license badge URL to use correct repository path
- [x] 4.5 Verify AUR badge URL is correct

## 5. Validation

- [ ] 5.1 Test CI workflow runs on a PR
- [ ] 5.2 Test release workflow with a test tag (can be deleted afterward)
- [ ] 5.3 Verify all artifacts are attached to the draft release
- [ ] 5.4 Verify version validation fails when version is not incremented
- [ ] 5.5 Verify README badges display correct status
