# Voice Transcription Design

## Context

OmniRec records screen/window/region with optional audio. This change adds real-time voice transcription to produce a timestamped markdown transcript alongside the video output. The implementation draws heavily from FlowSTT's proven approach while adapting for OmniRec's service architecture and longer-form transcription needs.

**Reference implementation**: `~/ws/flowstt` provides working code for:
- whisper.cpp FFI bindings via libloading
- Voice activity detection with multi-feature analysis
- Word-break detection for natural segment boundaries
- Ring buffer for continuous audio capture
- Transcription queue with worker thread

## Goals

- Transcribe voice from system audio during recording
- Produce accurate transcripts for longer speech segments (20+ seconds)
- Run transcription in the service process (decoupled from UI)
- No interference with video/audio recording quality or performance
- Ensure continuous audio flow with no missed words

## Non-Goals

- Real-time display of transcription in UI (future enhancement)
- Speaker diarization (who said what)
- Translation (transcription only, English)
- Custom vocabulary or fine-tuning

## Decisions

### 1. Whisper Model: medium.en

**Decision**: Use `ggml-medium.en.bin` (~1.5GB) exclusively.

**Rationale**:
- FlowSTT uses base.en for real-time short segments (<4s)
- OmniRec processes longer segments (up to 30s) where accuracy matters more
- medium.en provides significantly better accuracy for longer utterances
- English-only models are faster than multilingual equivalents
- Single model simplifies UX (no model selection needed)

**Alternatives considered**:
- base.en: Too low accuracy for longer segments
- large-v3: Diminishing returns vs. significant resource increase
- Configurable: Adds complexity without clear user benefit

### 2. Audio Pipeline Architecture

**Decision**: Fork audio stream to transcription module without affecting recording.

```
Audio Capture (PipeWire/WASAPI/CoreAudio)
    │
    ├─► Encoder (existing path, unchanged)
    │
    └─► Transcription Pipeline (new, when enabled)
            │
            ├─► Voice Detector (speech detection)
            │
            ├─► Segment Buffer (ring buffer, 30s capacity)
            │
            └─► Transcription Queue (async whisper processing)
```

**Rationale**:
- Recording quality must not be impacted
- Transcription is a read-only consumer of audio samples
- Service architecture allows background processing

### 3. Voice Detection (from FlowSTT)

**Decision**: Port FlowSTT's `SpeechDetector` with dual-mode detection.

**Features to port**:
- RMS amplitude detection with dB threshold
- Zero-Crossing Rate (ZCR) for voiced/whisper distinction
- Spectral centroid estimation (no FFT, first-difference method)
- Transient rejection (keyboard clicks, mouse sounds)
- Lookback buffer for true speech start detection
- Word-break detection for natural segment boundaries

**Tuning changes for longer segments**:
- Increase hold time from 300ms to 500ms (tolerate longer pauses)
- Word-break minimum gap: 150ms (vs 15ms in FlowSTT)
- Word-break maximum gap: 500ms (vs 200ms in FlowSTT)

### 4. Segment Buffering Strategy

**Decision**: 20s threshold, 2s grace period, 30s absolute maximum.

**Flow**:
1. Speech starts → begin segment, start duration counter
2. At 20s → enter "seeking word break" mode
3. If word break found within 2s → submit segment at word break
4. If no word break by 22s → force submit at current position
5. If speech continues past 30s without pause → force submit (safety)
6. After submission → continue segment from submission point (no gap)

**Ring buffer size**: 35 seconds at 48kHz stereo = 3,360,000 samples (~13MB)

**Rationale**:
- 20s is long enough for complete thoughts
- 2s grace period allows finding natural break points
- 30s prevents runaway segments during continuous speech
- Seamless continuation ensures no words are lost

### 5. Whisper Configuration for Long Segments

**Decision**: Configure whisper for longer segments (opposite of FlowSTT's short-segment tuning).

```rust
// Unlike FlowSTT's configure_for_short_audio(), we use:
params.no_context = false;         // Use context for better coherence
params.single_segment = false;     // Allow multiple segments per chunk
params.duration_ms = 0;            // Process full audio
params.max_tokens = 256;           // Allow longer output
params.entropy_thold = 2.4;        // Default (don't suppress low-entropy)
params.logprob_thold = -1.0;       // Default threshold
params.no_timestamps = false;      // Enable timestamps for output
```

### 6. Transcript Output Format

**Decision**: Markdown file with timestamps, one segment per line.

**Format**:
```markdown
# Transcript

[00:00:05] Hello and welcome to this tutorial.
[00:00:28] Today we're going to learn about screen recording.
[00:01:15] First, let's look at the settings panel.
```

**File naming**: `{video_basename}_transcript.md` (e.g., `recording_2024-01-15_143052_transcript.md`)

**Timestamp source**: Recording elapsed time when segment started, not whisper's internal timestamps.

### 7. Service Integration

**Decision**: Transcription runs in `src-service`, controlled via IPC.

**IPC commands**:
- `start_transcription(output_path)` - called when recording starts (if enabled)
- `stop_transcription()` - called when recording stops, finalizes transcript
- `get_transcription_status()` - returns model loaded, segments processed, etc.

**Audio delivery**: Service already receives mixed audio samples. Transcription module subscribes to same sample stream via channel clone.

### 8. CUDA/GPU Acceleration

**Decision**: Compile-time feature flag, matching FlowSTT approach.

**Build variants**:
- Default: CPU-only (portable, smaller binary)
- `--features cuda`: GPU-accelerated (requires CUDA toolkit on Linux, downloads prebuilt on Windows)

**Platform handling** (from FlowSTT):
- Linux: Build whisper.cpp from source with `-DGGML_CUDA=ON`
- Windows: Download prebuilt CUDA binaries (~457MB additional)
- macOS: Metal acceleration via prebuilt framework (always enabled, no CUDA)

### 9. Model Location and Download

**Decision**: Standard cache directory, manual download required initially.

**Model path**:
- Linux: `~/.cache/omnirec/whisper/ggml-medium.en.bin`
- macOS: `~/Library/Caches/omnirec/whisper/ggml-medium.en.bin`
- Windows: `%LOCALAPPDATA%\omnirec\whisper\ggml-medium.en.bin`

**Future enhancement**: Auto-download with progress indicator (not in initial scope).

### 10. UI Placement

**Decision**: Checkbox in two locations with conditional visibility.

**Settings (Audio group)**:
```
Audio
├── System Audio: [dropdown]
├── Microphone: [dropdown]
├── Echo Cancellation: [checkbox]
└── Transcribe Voice: [checkbox]  ← NEW
```

**Main UI (controls section)**:
```
┌─────────────────────────────────────────┐
│ [Record]  00:00     □ Transcribe voice  │
└─────────────────────────────────────────┘
```

**Visibility rule**: Main UI checkbox only visible when system audio is enabled (source selected or macOS checkbox checked). Settings checkbox always visible.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Large model download (1.5GB) | Clear documentation, future auto-download |
| GPU memory on low-end systems | CPU fallback always available |
| Transcription delays video save | Transcription finalizes async, video saves immediately |
| Poor accuracy in noisy audio | Document that clean audio works best |
| Whisper hallucinations on silence | Voice detection prevents processing silence |

## Migration Plan

No migration needed - new feature with opt-in setting.

## Open Questions

1. Should we show a progress indicator for model loading on first use?
2. Should transcript errors (e.g., model not found) block recording or just disable transcription silently?
3. Future: Real-time transcript preview in UI during recording?
