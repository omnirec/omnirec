<p align="center">
    <picture>
      <!-- <source srcset="images/omnirec-banner-dark.png" media="(prefers-color-scheme: dark)"> -->
      <!-- <source srcset="images/omnirec-banner-white.png" media="(prefers-color-scheme: light)"> -->
      <img src="images/omnirec-banner.png" alt="OmniRec logo">
    </picture>
</p>

<p align="center"><i>The universal screen recorder for every desktop</i></p>

<p align="center">
  <a href="https://github.com/omnirec/omnirec/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/omnirec/omnirec/ci.yml?branch=master&label=build" alt="Build Status"></a>
  <a href="https://github.com/omnirec/omnirec/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/omnirec/omnirec/ci.yml?branch=master&label=tests" alt="Test Status"></a>
  <a href="https://github.com/omnirec/omnirec/releases"><img src="https://img.shields.io/github/v/release/omnirec/omnirec" alt="Release"></a>
  <!-- <a href="https://aur.archlinux.org/packages/omnirec-bin"><img src="https://img.shields.io/aur/version/omnirec-bin" alt="AUR Version"></a> -->
</p>

<picture><img src="images/omnirec-screenshots.gif"></picture>

---

## Key Features

- **Universal Capture** — Record any window, entire display, or custom screen region
- **Audio Recording** — Capture system audio, microphone, or both with dual-source mixing
- **Echo Cancellation** — Built-in AEC removes speaker feedback when recording with a microphone
- **Multiple Formats** — Export to MP4, WebM, MKV, QuickTime, GIF, APNG, or WebP
- **Cross-Platform** — Works on Windows, macOS (12.3+), and Linux (Hyprland, GNOME, KDE, COSMIC)
- **Privacy-First** — All processing happens locally — your recordings never leave your machine
- **Free & Open Source** — No subscriptions, no accounts, no limits

## Coming Soon

- **Voice Transcription** — Streaming speech-to-text for automatic captions and searchable recordings
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

## Command Line Interface

OmniRec includes a CLI (`omnirec-cli`) for headless recording and automation.

### Quick Start

```bash
# List available capture sources
omnirec-cli list windows
omnirec-cli list displays
omnirec-cli list audio

# Record a window (use handle from 'list windows')
omnirec-cli record window 12345

# Record a display (use ID from 'list displays')
omnirec-cli record display 0

# Record a region
omnirec-cli record region --display 0 --x 100 --y 100 --width 800 --height 600

# Stop recording (or press Ctrl+C)
omnirec-cli stop

# Check recording status
omnirec-cli status
```

### Recording Options

```bash
# Specify output format
omnirec-cli record window 12345 --format webm

# Auto-stop after duration
omnirec-cli record display 0 --duration 60

# Configure audio sources
omnirec-cli record window 12345 --audio <source-id> --microphone <mic-id>

# Disable audio
omnirec-cli record display 0 --audio none --microphone none
```

### JSON Output

Use `--json` for machine-readable output (useful for scripts):

```bash
omnirec-cli list displays --json
omnirec-cli status --json
```

### Wayland Desktop Environments

On GNOME, KDE Plasma, COSMIC, and Cinnamon (Wayland), specific window/display/region selection is not supported due to Wayland security restrictions. Use portal-based recording instead:

```bash
# Opens the desktop's native screen picker
omnirec-cli record portal
```

When using `record window`, `display`, or `region` on these desktops, the CLI will warn and fall back to portal mode. Use `--strict` to fail instead of falling back.

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Service connection failed |
| 4 | Recording failed to start |
| 5 | Recording failed during capture |
| 6 | Transcoding failed |
| 7 | Portal required (with --strict) |
| 8 | User cancelled |

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
