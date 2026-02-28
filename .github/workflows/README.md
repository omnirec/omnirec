# GitHub Actions Workflows

OmniRec uses GitHub Actions for CI/CD. Workflows run automatically on push to `master` and pull requests.

## Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push to `master`, PRs | Lint, type-check, and test |
| `release.yml` | Tag push (`v*`) | Build and publish releases |

## Releasing

Releases are driven by the `scripts/release.mjs` script:

```bash
node scripts/release.mjs v0.2.0
```

This validates the version increment, updates all versioned files, commits, pushes, creates the git tag, and pushes the tag. The tag push triggers the release workflow.

Use `--dry-run` to apply file updates without any git commands:

```bash
node scripts/release.mjs v0.2.0 --dry-run
```

## Local CI Testing with `act`

You can run GitHub Actions workflows locally using [act](https://github.com/nektos/act):

```bash
# Install act (macOS)
brew install act

# Install act (Linux)
curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash
```

```bash
# Run CI workflow (Linux job only - requires Docker)
act push -j lint-and-test --matrix platform:ubuntu-22.04

# Dry run (list jobs without executing)
act push -l
```

> **Note:** Local CI testing requires Docker. Some platform-specific steps (macOS, Windows) cannot be tested locally with `act`.

## CUDA Builds

CUDA-accelerated builds are **not** run in CI because GitHub-hosted runners lack NVIDIA GPUs. The `cuda` feature is excluded from clippy and test runs:

```yaml
# src-tauri is linted/tested without --all-features
cargo clippy -p omnirec --all-targets -- -D warnings  # No cuda feature
cargo test -p omnirec                                   # No cuda feature
```

To test CUDA builds locally:

```bash
# Build with CUDA (requires NVIDIA CUDA Toolkit on Linux)
make build-cuda

# Or directly:
cargo build -p omnirec --release --features cuda
```

## Platform Matrix

### CI (`ci.yml`)

| Platform | Runner | Notes |
|----------|--------|-------|
| macOS | `macos-latest` | - |
| Windows | `windows-latest` | - |

### Release (`release.yml`)

| Platform | Runner | Target | Notes |
|----------|--------|--------|-------|
| macOS | `macos-latest` | `aarch64-apple-darwin` | Apple Silicon |
| macOS | `macos-latest` | `x86_64-apple-darwin` | Intel |
| Windows | `windows-latest` | `x86_64-pc-windows-msvc` | - |

## Troubleshooting

**Missing secrets:** Some workflows require secrets. Use `-s SECRET_NAME=value` or create a `.secrets` file.

**Large images:** First run downloads large Docker images (~18GB for full Ubuntu runner). Use `-P ubuntu-22.04=catthehacker/ubuntu:act-22.04` for smaller images.

**ARM Macs:** Add `--container-architecture linux/amd64` if you encounter architecture issues.
