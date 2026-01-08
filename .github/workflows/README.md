# GitHub Workflows

This directory contains GitHub Actions workflows for CI and releases.

## Workflows

- **ci.yml** - Runs on push/PR to master. Performs linting and tests on all platforms.
- **release.yml** - Runs on version tags (`v*.*.*`). Builds and publishes releases.

## Local Testing with Act

[Act](https://github.com/nektos/act) allows you to run GitHub Actions locally using Docker.

### Installation

```bash
# Arch Linux
paru -S act

# macOS
brew install act

# Other platforms
# See: https://nektosact.com/installation/
```

### Running Workflows

**Run CI workflow (Linux job only):**

```bash
act push -j lint-and-test --matrix platform:ubuntu-22.04
```

**Dry run (list jobs without executing):**

```bash
act push -l
```

**Run with specific event:**

```bash
act pull_request -j lint-and-test --matrix platform:ubuntu-22.04
```

### Platform Limitations

Act runs workflows in Docker containers, which means:

- **Linux jobs** work well and are the primary use case
- **macOS/Windows jobs** cannot run locally (use `--matrix platform:ubuntu-22.04` to filter)

### Common Options

| Option | Description |
|--------|-------------|
| `-j <job>` | Run specific job |
| `-l` | List available jobs |
| `-n` | Dry run (don't execute) |
| `--matrix key:value` | Filter matrix to specific values |
| `-v` | Verbose output |
| `--container-architecture linux/amd64` | Force architecture (useful on ARM) |

### Example: Test CI Before Pushing

```bash
# Run the full CI lint-and-test job for Linux
act push -j lint-and-test --matrix platform:ubuntu-22.04

# Or just list what would run
act push -l
```

### Testing the Release Workflow

The release workflow supports `workflow_dispatch` for local testing without creating actual GitHub releases:

```bash
# Build packages locally (skips tests, version validation, and GitHub release)
act workflow_dispatch -j build-and-release \
  --matrix platform:ubuntu-22.04 \
  --input version=0.0.0-local \
  --input skip_tests=true

# Run with tests included
act workflow_dispatch -j build-and-release \
  --matrix platform:ubuntu-22.04 \
  --input version=0.0.0-local \
  --input skip_tests=false
```

When run via `workflow_dispatch`:
- Version validation is skipped
- Tests can be skipped with `skip_tests=true`
- Builds packages using `pnpm tauri build` instead of tauri-action
- Does not create or upload to GitHub releases
- Lists build artifacts at the end

## CUDA Builds

The release workflow includes a `build-linux-cuda` job for GPU-accelerated transcription. This requires a self-hosted runner with:

- NVIDIA GPU
- CUDA Toolkit installed (`nvcc` available in PATH)
- Runner labels: `self-hosted`, `linux`, `cuda`

**Setting up a self-hosted runner:**

1. On your CUDA-capable machine, follow [GitHub's self-hosted runner setup](https://docs.github.com/en/actions/hosting-your-own-runners/adding-self-hosted-runners)
2. Add the labels `linux` and `cuda` to the runner
3. Ensure CUDA toolkit is installed: `nvcc --version`

**Triggering CUDA builds:**

```bash
# Via GitHub CLI
gh workflow run release.yml \
  --field version=0.1.0 \
  --field skip_tests=true \
  --field build_cuda=true

# Or use the GitHub Actions UI with workflow_dispatch
```

For tag-triggered releases, the CUDA job runs automatically if a matching self-hosted runner is available. If no runner is available, the job will queue and wait (or fail after timeout).

**Output:** Creates `omnirec-VERSION-linux-x86_64-cuda.tar.gz` with GPU-accelerated whisper.cpp.

### Troubleshooting

**Missing secrets:** Some workflows require secrets. Use `-s SECRET_NAME=value` or create a `.secrets` file.

**Large images:** First run downloads large Docker images (~18GB for full Ubuntu runner). Use `-P ubuntu-22.04=catthehacker/ubuntu:act-22.04` for smaller images.

**ARM Macs:** Add `--container-architecture linux/amd64` if you encounter architecture issues.
