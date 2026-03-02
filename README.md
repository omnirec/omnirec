<p align="center">
    <picture>
      <!-- <source srcset="images/omnirec-banner-dark.png" media="(prefers-color-scheme: dark)"> -->
      <!-- <source srcset="images/omnirec-banner-white.png" media="(prefers-color-scheme: light)"> -->
      <img src="images/omnirec-banner.png" alt="OmniRec logo">
    </picture>
</p>

<p align="center"><i>The universal screen recorder for every desktop</i></p>

[![CI](https://github.com/omnirec/omnirec/actions/workflows/ci.yml/badge.svg)](https://github.com/omnirec/omnirec/actions/workflows/ci.yml)
[![Release](https://github.com/omnirec/omnirec/actions/workflows/release.yml/badge.svg)](https://github.com/omnirec/omnirec/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](https://github.com/omnirec/omnirec/blob/master/LICENSE)

<picture><img src="images/omnirec-screenshots.gif"></picture>

---

## Key Features

- **Universal Capture** — Record any window, entire display, or custom screen region
- **Audio Recording** — Capture system audio, microphone, or both with dual-source mixing
- **Echo Cancellation** — Built-in AEC removes speaker feedback when recording with a microphone
- **Voice Transcription** — Automatic speech-to-text creates timestamped transcripts alongside your recordings
- **Multiple Formats** — Export to MP4, WebM, MKV, QuickTime, GIF, APNG, or WebP
- **Cross-Platform** — Works on Windows, macOS (12.3+), and Linux (Hyprland, GNOME, KDE, COSMIC)
- **Privacy-First** — All processing happens locally — your recordings never leave your machine
- **Free & Open Source** — No subscriptions, no accounts, no limits

## Coming Soon

- **Global Hotkeys** — Start, stop, and pause recordings from anywhere with customizable shortcuts

## Use Cases

| Use Case | Description |
|----------|-------------|
| **Record Meetings** | Capture video calls from Zoom, Teams, or Google Meet with system audio and microphone. Echo cancellation prevents feedback when using speakers. |
| **Create Tutorials** | Record step-by-step walkthroughs of software, websites, or workflows with voiceover narration. |
| **Capture Gameplay** | Record gaming sessions with system audio for sharing highlights or streaming clips. |
| **Bug Reporting** | Quickly capture and share screen recordings to demonstrate issues to developers or support teams. |

## Supported Platforms

| Platform | Version |
|----------|---------|
| Windows | 10, 11 |
| macOS | 12.3+ |
| Linux | Arch, Debian, Ubuntu, Fedora, and others |

## Installation

> [!WARNING]
>
> OmniRec is in active development and is not yet available for general use. If you would like to help test OmniRec or contribute to its development, please download one of the available pre-release installation packages or build from source (see below).

<!-- release-links:start -->
**Current release:** v0.1.15

**Download packages**
- Windows: [OmniRec_0.1.15_x64-setup.exe](https://github.com/omnirec/omnirec/releases/download/v0.1.15/OmniRec_0.1.15_x64-setup.exe)
- macOS (Apple Silicon M-Series): [OmniRec_aarch64.dmg](https://github.com/omnirec/omnirec/releases/download/v0.1.15/OmniRec_aarch64.dmg)
- macOS (Legacy Intel x64): [OmniRec_x64.dmg](https://github.com/omnirec/omnirec/releases/download/v0.1.15/OmniRec_x64.dmg)
- Linux: See the [Releases page](https://github.com/omnirec/omnirec/releases) for `.deb`, `.rpm`, and other packages
<!-- release-links:end -->

### Windows

Download the latest `.msi` installer from the [Releases](https://github.com/omnirec/omnirec/releases) page.

### macOS

Download the latest `.dmg` from the [Releases](https://github.com/omnirec/omnirec/releases) page, open it, and drag OmniRec to your Applications folder.

### Linux

#### Arch

Use `makepkg`:

```bash
curl -LO https://github.com/omnirec/omnirec/releases/latest/download/PKGBUILD
curl -LO https://github.com/omnirec/omnirec/releases/latest/download/omnirec.desktop
makepkg -si
```

#### Debian/Ubuntu/Pop!_OS

Download the latest `.deb` package from the [Releases](https://github.com/omnirec/omnirec/releases) page. 

#### Fedora

Download the latest `.rpm` package from the [Releases](https://github.com/omnirec/omnirec/releases) page. 

## Configuration & Permissions

OmniRec is designed to work with zero configuration, but it is necessary to grant it permission to record your screen. This process varies by OS and desktop environment:

### macOS

On first launch, grant Screen Recording permission in System Settings > Privacy & Security.

### Linux

#### Hyprland

On first recording request, OmniRec will display an approval dialog asking for permission to record the screen. You can check "Always allow OmniRec to record the screen" to bypass this dialog for future recordings.

The approval token is stored at `~/.local/state/omnirec/approval-token`. To revoke permission, delete this file.

#### GNOME

On GNOME, OmniRec runs as a system tray application. Click the tray icon to access recording controls. The standard system share interface is used to choose the recording source. Due to limitations of system share source selection diaglog, region selection and recording is not supported in GNOME.

> [!NOTE]
>
> Debian users must install and enable the app indicator GNOME extension:
>
> ```
> sudo apt install gnome-shell-extension-appindicator
> ```
>
> Then log out, log in again and enable it with:
>
> ```
> gnome-extensions enable ubuntu-appindicators@ubuntu.com
> ```

#### KDE Plasma

OmniRec is accessed from the taskbar in KDE Plasma.

#### COSMIC

On Pop!_OS with the COSMIC desktop environment, OmniRec runs as a system tray application similar to GNOME.

## Voice Transcription

OmniRec can automatically transcribe speech during recording, creating a timestamped markdown transcript alongside your video file.

### Enabling Transcription

1. Enable **System Audio** in Settings (transcription requires system audio)
2. Check the **Transcribe voice** option in Settings or use the quick toggle on the record button row
3. Record as normal — the transcript will be saved as `{video_name}_transcript.md`

### Whisper Model

Transcription uses [whisper.cpp](https://github.com/ggerganov/whisper.cpp) with the `ggml-medium.en` model (~1.5GB). The model is downloaded automatically on first use and cached in:

| Platform | Location |
|----------|----------|
| Linux | `~/.cache/omnirec/ggml-medium.en.bin` |
| macOS | `~/Library/Caches/omnirec/ggml-medium.en.bin` |
| Windows | `%LOCALAPPDATA%\omnirec\cache\ggml-medium.en.bin` |

### GPU Acceleration (CUDA)

For faster transcription on NVIDIA GPUs, build with CUDA support:

```bash
# Build with CUDA acceleration (Linux only)
cargo build -p omnirec --features cuda --release

# Or use the Makefile target
make build-cuda
```

Requires CUDA toolkit installed on your system.

## Command Line Interface

OmniRec includes a CLI (`omnirec`) for headless recording and automation. See [CLI Documentation](docs/cli.md) for complete reference.

### Quick Start (Windows, macOS, Hyprland)

```bash
# List available capture sources
omnirec list windows
omnirec list displays

# Record a window, display, or region
omnirec record window 12345
omnirec record display 0
omnirec record region --display 0 --x 100 --y 100 --width 800 --height 600

# Stop recording (or press Ctrl+C)
omnirec stop
```

### Quick Start (GNOME, KDE, COSMIC)

Portal-based desktops use the system's native screen picker:

```bash
# Start recording (opens native picker)
omnirec record portal

# Stop recording (or press Ctrl+C)
omnirec stop
```

### Recording Options

```bash
# Specify output format
omnirec record portal --format webm

# Auto-stop after duration
omnirec record portal --duration 60

# Configure audio sources
omnirec record portal --audio <source-id> --microphone <mic-id>
```

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) (LTS recommended)
- [pnpm](https://pnpm.io/) (optional)
- [Rust](https://rustup.rs/)
- [ImageMagick](https://imagemagick.org/script/download.php) (for icon generation)

### Build & Run

OmniRec is built with [Tauri](https://v2.tauri.app/). Use `npm` (or equivalent) to install and run the dev server:

```
cd omnirec
npm install
npm run tauri dev
```

## License

[MIT](LICENSE)
