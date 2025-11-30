# Screen Recorder

A high-performance screen, window, and region recording application built with Tauri. Record and share anything — completely free, no cloud account required.

> **Status**: Early Alpha — Windows only. Core recording functionality works, but many planned features are not yet implemented.

## Current Features

- **Window Recording**: Capture any application window using Windows.Graphics.Capture API
- **Region Recording**: Select and record a custom region of your screen
- **MP4 Output**: H.264 encoded video via FFmpeg
- **High Performance**: Native Rust backend with efficient frame pipeline
- **No Cloud Required**: All processing happens locally — your recordings stay on your machine
- **Free & Open Source**: No subscriptions, no accounts, no limits

### Output

- **Format**: MP4 (H.264)
- **Frame Rate**: 30 fps (fixed)
- **Location**: User's Videos folder
- **Filename**: `recording_YYYY-MM-DD_HHMMSS.mp4`

### Planned Features

See [docs/requirements.md](docs/requirements.md) for the full roadmap, including:

- Full screen capture
- Pause/Resume recording
- Audio capture (system + microphone)
- Additional output formats (WebM, GIF, APNG, WebP)
- Configurable quality and frame rate
- Global hotkeys
- macOS and Linux support

## Tech Stack

- **Frontend**: Vanilla TypeScript, HTML, CSS
- **Backend**: Rust + Tauri v2
- **Capture**: [windows-capture](https://crates.io/crates/windows-capture) (Windows.Graphics.Capture API)
- **Encoding**: [ffmpeg-sidecar](https://crates.io/crates/ffmpeg-sidecar) (auto-downloads FFmpeg)
- **Build**: Vite

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri Prerequisites](https://tauri.app/v2/start/prerequisites/) (Windows: WebView2, VS Build Tools)

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
│   ├── main.ts             # Main application logic
│   ├── selection-overlay.ts # Region selection UI
│   └── styles.css          # App styles (dark mode support)
├── src-tauri/              # Rust backend
│   └── src/
│       ├── lib.rs          # Tauri commands
│       ├── state.rs        # Recording state management
│       ├── capture/        # Window/region capture modules
│       └── encoder/        # FFmpeg encoding
├── docs/                   # Documentation
│   └── requirements.md     # Full project requirements
├── openspec/               # Project specifications
└── package.json            # Node.js dependencies
```

## Documentation

- [Requirements](docs/requirements.md) - Full project requirements and specifications

## License

[MIT](LICENSE)
