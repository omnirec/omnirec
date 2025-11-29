# Screen Recorder

A high-performance screen, window, and region recording application built with Tauri. Record and share anything in a variety of formats — completely free, no cloud account required.

## Features

- **Multiple Capture Modes**: Record your entire screen, specific windows, or custom regions
- **High Performance**: Native Rust backend ensures minimal resource usage and smooth recording
- **Multiple Output Formats**: Export to MP4, WebM, MKV, GIF, Animated PNG, WebP, and more
- **No Cloud Required**: All processing happens locally — your recordings stay on your machine
- **Free & Open Source**: No subscriptions, no accounts, no limits

### Supported Output Formats

| Category | Formats |
|----------|---------|
| Video | MP4 (H.264/H.265), WebM (VP9), MKV, AVI, MOV |
| Animated Image | GIF, Animated PNG (APNG), Animated WebP |
| Image Sequence | PNG, JPEG |

See [docs/requirements.md](docs/requirements.md) for full specifications.

## Tech Stack

- **Frontend**: Vanilla TypeScript, HTML, CSS
- **Backend**: Rust + Tauri v2
- **Build**: Vite

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

## Development Setup

```bash
# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [Tauri Extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Project Structure

```
screen-recorder/
├── src/                    # Frontend TypeScript/HTML/CSS
├── src-tauri/              # Rust backend
│   ├── src/                # Rust source code
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── docs/                   # Documentation
│   └── requirements.md     # Full project requirements
├── openspec/               # Project specifications
└── package.json            # Node.js dependencies
```

## Documentation

- [Requirements](docs/requirements.md) - Full project requirements and specifications

## License

[MIT](LICENSE)
