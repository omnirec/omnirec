<picture>
  <!-- <source srcset="images/omnirec-banner-dark.png" media="(prefers-color-scheme: dark)"> -->
  <!-- <source srcset="images/omnirec-banner-white.png" media="(prefers-color-scheme: light)"> -->
  <img src="images/omnirec-banner.png" alt="OmniRec logo">
</picture>

<p align="center"><i>The universal screen recorder for every desktop</i></p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/github/actions/workflow/status/user/omnirec/build.yml?label=build" alt="Build Status"></a>
  <a href="#"><img src="https://img.shields.io/github/actions/workflow/status/user/omnirec/test.yml?label=tests" alt="Test Status"></a>
  <a href="#"><img src="https://img.shields.io/github/v/release/user/omnirec" alt="Release"></a>
  <a href="#"><img src="https://img.shields.io/github/license/user/omnirec" alt="License"></a>
  <a href="#"><img src="https://img.shields.io/aur/version/omnirec-bin" alt="AUR Version"></a>
</p>

<picture><img src="images/omnirec-screenshots.gif"></picture>

---

## Key Features

- 🖥️ **Universal Capture** — Record any window, entire display, or custom screen region
- 🔊 **Audio Recording** — Capture system audio, microphone, or both with dual-source mixing
- 🔇 **Echo Cancellation** — Built-in AEC removes speaker feedback when recording with a microphone
- 📦 **Multiple Formats** — Export to MP4, WebM, MKV, QuickTime, GIF, APNG, or WebP
- 🌍 **Cross-Platform** — Works on Windows, macOS (12.3+), and Linux (Hyprland/Wayland)
- 🔒 **Privacy-First** — All processing happens locally — your recordings never leave your machine
- 💚 **Free & Open Source** — No subscriptions, no accounts, no limits

## Coming Soon

- 🗣️ **Voice Transcription** — Streaming speech-to-text for automatic captions and searchable recordings
- ⌨️ **Global Hotkeys** — Start, stop, and pause recordings from anywhere with customizable shortcuts
- 💻 **Command Line Interface** — Scriptable recording for automation and power users

## Use Cases

| | Use Case | Description |
|---|----------|-------------|
| 📹 | **Record Meetings** | Capture video calls from Zoom, Teams, or Google Meet with system audio and microphone. Echo cancellation prevents feedback when using speakers. |
| 🎓 | **Create Tutorials** | Record step-by-step walkthroughs of software, websites, or workflows with voiceover narration. |
| 🎮 | **Capture Gameplay** | Record gaming sessions with system audio for sharing highlights or streaming clips. |
| 🐛 | **Bug Reporting** | Quickly capture and share screen recordings to demonstrate issues to developers or support teams. |

## Supported Platforms

| Platform | Version |
|----------|---------|
| Windows | 10, 11 |
| macOS | 12.3+ |
| Linux | Arch, Debian, Ubuntu, Fedora, and others |

**Linux Desktop Environments** (Wayland): Hyprland, GNOME, KDE

## Installation

### Windows

Download the latest `.msi` installer from the [Releases](https://github.com/user/omnirec/releases) page.

### macOS

Download the latest `.dmg` from the [Releases](https://github.com/user/omnirec/releases) page, open it, and drag OmniRec to your Applications folder.

> **Note**: On first launch, grant Screen Recording permission in System Settings > Privacy & Security.

### Linux (Arch/AUR)

```bash
# Using an AUR helper
yay -S omnirec-bin

# Or with paru
paru -S omnirec-bin
```

### Linux (Other Distros)

Download the latest `.AppImage` or `.deb` from the [Releases](https://github.com/user/omnirec/releases) page.

```bash
# AppImage
chmod +x OmniRec-*.AppImage
./OmniRec-*.AppImage

# Debian/Ubuntu
sudo dpkg -i omnirec_*.deb
```

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) (LTS recommended)
- [pnpm](https://pnpm.io/) (optional)
- [Rust](https://rustup.rs/)
- [ImageMagick](https://imagemagick.org/script/download.php) (for icon generation)

### Regenerating Icons

To regenerate all application icons from the source SVG (`images/omnirec-icon.svg`):

```bash
pnpm icons:generate
```

This script uses ImageMagick to convert the SVG to a high-resolution PNG, then generates all platform-specific icons (Windows .ico, macOS .icns, iOS, Android, and various PNG sizes) using Tauri's built-in icon generator.

## License

[MIT](LICENSE)
