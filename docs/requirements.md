# Screen Recorder Requirements

## Overview

Screen Recorder is a high-performance desktop application for capturing screens, windows, and custom regions. It enables users to easily record and share content in various formats without requiring cloud services or accounts.

## Goals

- **High Performance**: Minimal CPU/memory overhead during recording
- **Ease of Use**: Simple, intuitive interface for quick captures
- **Privacy First**: All processing local, no cloud dependencies or telemetry
- **Format Flexibility**: Export to multiple video and image formats
- **Free & Open**: No subscriptions, accounts, or artificial limitations

## Functional Requirements

### Capture Modes

| Mode | Description |
|------|-------------|
| **Full Screen** | Capture entire display (single or multi-monitor support) |
| **Window** | Capture a specific application window |
| **Region** | User-defined rectangular area selection |

### Recording Features

- **Start/Stop/Pause**: Basic recording controls
- **Hotkeys**: Global keyboard shortcuts for recording control
- **Audio Capture**: System audio and/or microphone input (optional)
- **Frame Rate**: Configurable FPS (15, 30, 60)
- **Quality Presets**: Low, Medium, High, Lossless options
- **Recording Timer**: Optional countdown before recording starts
- **Duration Limit**: Optional maximum recording length

### Output Formats

The application supports multiple output formats via FFmpeg integration:

#### Video Formats

| Format | Container | Codec | Use Case |
|--------|-----------|-------|----------|
| **MP4** | .mp4 | H.264 (libx264) | Universal compatibility, good compression |
| **MP4 (HEVC)** | .mp4 | H.265 (libx265) | Better compression, newer devices |
| **WebM** | .webm | VP9 (libvpx-vp9) | Web-optimized, open format |
| **MKV** | .mkv | H.264/H.265 | Flexible container, preserves quality |
| **AVI** | .avi | Various | Legacy compatibility |
| **MOV** | .mov | H.264/ProRes | macOS/Apple ecosystem |

#### Animated Image Formats

| Format | Extension | Use Case |
|--------|-----------|----------|
| **GIF** | .gif | Simple animations, universal support, limited colors |
| **Animated PNG (APNG)** | .png | Full color, transparency, larger files |
| **Animated WebP** | .webp | Good compression, modern browser support |

#### Image Sequence Export

| Format | Extension | Use Case |
|--------|-----------|----------|
| **PNG Sequence** | .png | Lossless frames for editing |
| **JPEG Sequence** | .jpg | Compressed frames |

### Post-Recording Features

- **Trim**: Cut start/end of recording
- **Convert**: Change output format after recording
- **Preview**: Play back recording before saving
- **Quick Share**: Copy to clipboard, save to file, or open save location

### User Interface

- **Source Selection**: Visual picker for screens, windows, regions
- **Recording Indicator**: Clear visual feedback during capture
- **Preview Window**: Optional live preview during recording
- **Settings Panel**: Format, quality, audio, hotkey configuration
- **Recording History**: List of recent recordings with quick access

## Non-Functional Requirements

### Performance

- Recording should use <10% CPU on modern hardware (targeting 1080p@30fps)
- Memory usage should stay under 500MB during recording
- No frame drops at target frame rate under normal conditions
- Startup time <3 seconds

### Privacy & Security

- No network connections except user-initiated (optional update checks)
- No telemetry or analytics collection
- No account or registration required
- All processing happens locally on user's machine
- Recordings are never uploaded without explicit user action

### Platform Support

| Platform | Minimum Version | Capture API |
|----------|-----------------|-------------|
| Windows | Windows 10 | DXGI Desktop Duplication |
| macOS | macOS 12.3+ | ScreenCaptureKit |
| Linux | Ubuntu 22.04+ | PipeWire / X11 |

### Accessibility

- Keyboard navigation support
- Screen reader compatibility
- High contrast theme support
- Configurable UI scaling

### Localization

- English (default)
- Internationalization support for future translations

## Technical Requirements

### Encoding Library

The application uses FFmpeg for video encoding, accessed via Rust bindings:

- **Primary**: `ffmpeg-sidecar` - Bundles FFmpeg binary, spawns as subprocess
- **Alternative**: `ez-ffmpeg` or native FFmpeg bindings for tighter integration

### Supported Codecs (via FFmpeg)

**Video Codecs:**
- libx264 (H.264/AVC) - Primary codec for MP4
- libx265 (H.265/HEVC) - Better compression option
- libvpx-vp9 (VP9) - WebM format
- gif - Animated GIF output
- apng - Animated PNG output
- libwebp - Animated WebP output

**Audio Codecs:**
- AAC - Default for MP4/MOV
- Opus - WebM audio
- MP3 - Legacy compatibility

### File Size Guidelines

| Quality | Resolution | FPS | Approximate Size (1 min) |
|---------|------------|-----|--------------------------|
| Low | 720p | 15 | ~5-10 MB |
| Medium | 1080p | 30 | ~20-40 MB |
| High | 1080p | 60 | ~50-100 MB |
| Lossless | 1080p | 30 | ~500+ MB |

## Future Considerations

- Hardware acceleration (NVENC, QuickSync, VideoToolbox)
- Webcam overlay support
- Annotation tools (draw on screen during recording)
- Scheduled recordings
- Streaming output (RTMP)
- Plugin/extension system
