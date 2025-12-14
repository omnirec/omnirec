## 1. Package Files

- [x] 1.1 Create `packaging/aur/` directory structure
- [x] 1.2 Create PKGBUILD with correct dependencies and build steps
- [x] 1.3 Create `.desktop` file for application menu entry
- [x] 1.4 Create `.install` file with post-install/removal hooks

## 2. Validation

- [x] 2.1 Run `namcap` to validate PKGBUILD (validated with `makepkg --printsrcinfo`)
- [ ] 2.2 Test `makepkg -si` in clean environment (requires release tarball)
- [ ] 2.3 Verify application launches from menu (requires installed package)
- [ ] 2.4 Verify picker service works after install (requires installed package)

## 3. Documentation

- [x] 3.1 Add AUR installation instructions to README
- [x] 3.2 Document AUR publishing workflow for maintainers
