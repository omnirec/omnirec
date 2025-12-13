## Context

OmniRec currently records directly to MP4 (H.264) format. Users have expressed the need for additional output formats to support various workflows:
- Web developers need WebM for browser-native video
- Social media users need GIF/animated formats for easy sharing
- Editors need MKV for maximum compatibility
- Apple ecosystem users need QuickTime (.mov)

The challenge is balancing recording performance (MP4/H.264 is highly optimized) with format flexibility.

## Goals / Non-Goals

**Goals:**
- Support 7 output formats: MP4, WebM, MKV, QuickTime, GIF, Animated PNG, Animated WebP
- Maintain current recording performance (no impact during capture)
- Retain original high-quality recording for future re-exports
- Provide clear progress feedback during transcoding

**Non-Goals:**
- Custom codec settings (future enhancement)
- Batch re-export of existing recordings (future enhancement)
- Live streaming to formats (out of scope)

## Decisions

### Decision 1: Record-then-transcode approach

**What:** Always record in MP4 (H.264) first, then transcode to target format if needed.

**Why:** 
- H.264 encoding is GPU-accelerated and highly optimized for real-time capture
- Transcoding can happen in background without time pressure
- User retains high-quality source for future re-exports

**Alternatives considered:**
- Direct encoding to target format: Rejected due to performance impact for formats like WebM (VP9) which are slower to encode
- Parallel encoding: Rejected due to complexity and resource usage

### Decision 2: FFmpeg-based transcoding

**What:** Use the existing FFmpeg sidecar for transcoding operations.

**Why:**
- Already bundled with the application
- Supports all target formats
- Well-tested and reliable

### Decision 3: Retain original MP4

**What:** Keep the original high-quality MP4 file alongside the transcoded output.

**Why:**
- User can re-export to different formats without quality loss
- No data loss if transcoding fails
- Minimal storage impact compared to recordings

**File naming convention:**
- Original: `recording_2024-01-15_143052.mp4`
- Transcoded: `recording_2024-01-15_143052.webm` (same base name, different extension)

### Decision 4: Format-specific encoding settings

**What:** Use optimized encoding settings for each format:

| Format | Codec | Settings |
|--------|-------|----------|
| MP4 | H.264 | Current settings (no transcoding) |
| WebM | VP9 | `-c:v libvpx-vp9 -crf 30 -b:v 0` |
| MKV | H.264 | `-c:v copy` (remux only, very fast) |
| QuickTime | H.264 | `-c:v copy -f mov` (remux only) |
| GIF | GIF | `-vf "fps=15,scale=640:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse"` |
| APNG | APNG | `-plays 0 -f apng` |
| WebP | WebP | `-c:v libwebp -lossless 0 -q:v 75 -loop 0` |

**Why:** Balanced quality/size for each format's typical use case.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Transcoding time for long recordings | Show progress bar, allow cancellation (future) |
| Large file sizes for animated formats | Warn user for recordings >30 seconds when selecting GIF/APNG/WebP |
| Disk space for original + transcoded | Document behavior, let user delete original manually |
| FFmpeg transcoding failure | Graceful error handling, original MP4 preserved |

## Migration Plan

No migration needed - this is additive functionality. Existing recordings continue to work.

## Open Questions

1. Should there be a setting to auto-delete original MP4 after successful transcoding? (Decided: No, keep for v1)
2. Should animated formats have a duration limit? (Decided: Warn only, don't block)
