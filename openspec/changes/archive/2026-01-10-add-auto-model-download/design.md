## Context

OmniRec uses whisper.cpp for voice transcription. Currently the model file must be manually downloaded and placed in a platform-specific location. Users have no visibility into whether the model exists or what to do when transcription fails. This change adds model management to improve usability.

## Goals / Non-Goals

**Goals:**
- Allow users to choose from available whisper models (tiny through large-v3)
- Provide in-app model download with progress feedback
- Show clear status of model availability

**Non-Goals:**
- Automatic background download without user action
- Support for custom/fine-tuned models
- Model caching or version management

## Decisions

### Model Selection

**Decision:** Provide English-only (.en suffix) and multilingual variants for sizes: tiny, base, small, medium, large-v3.

**Rationale:** English-only models are faster and more accurate for English content. Multilingual models support other languages. Large-v3-turbo omitted as it requires different handling. Quantized variants omitted to keep UI simple.

**Available Models:**

| Model | English-only | Multilingual | Size |
|-------|-------------|--------------|------|
| tiny | tiny.en | tiny | 75 MB |
| base | base.en | base | 142 MB |
| small | small.en | small | 466 MB |
| medium | medium.en | medium | 1.5 GB |
| large-v3 | n/a | large-v3 | 2.9 GB |

### Download Source

**Decision:** Download from Hugging Face: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`

**Rationale:** Official repository maintained by whisper.cpp author. Reliable CDN-backed hosting. Direct download links available.

### Model Storage

**Decision:** Store models in existing platform-specific cache directory with model-specific filenames.

- Linux: `~/.cache/omnirec/whisper/ggml-{model}.bin`
- macOS: `~/Library/Caches/omnirec/whisper/ggml-{model}.bin`
- Windows: `%LOCALAPPDATA%\omnirec\whisper\ggml-{model}.bin`

**Rationale:** Consistent with existing model path convention. Multiple models can coexist. Easy to find and manage.

### Download Implementation

**Decision:** Use async download with progress events via Tauri event system.

**Rationale:** Large files (up to 2.9GB) require progress indication. Event-based progress allows responsive UI updates. Cancel support requires async architecture.

```rust
// Event payload structure
struct DownloadProgress {
    model: String,
    bytes_downloaded: u64,
    total_bytes: u64,
    percentage: u8,
}
```

### Recording Gate

**Decision:** Prevent recording start if transcription is enabled but model is not downloaded. Show clear error message directing user to settings.

**Rationale:** Silent transcription failure creates confusion. Explicit blocking with guidance is more user-friendly than silent degradation.

## Risks / Trade-offs

**Risk:** Large download sizes (up to 2.9GB) may fail on slow/unstable connections.
**Mitigation:** Progress indication, cancel option, and resume support in future iteration.

**Risk:** Model files consume significant disk space.
**Mitigation:** Show size clearly before download. Only one model downloaded at a time (unless user explicitly downloads multiple).

## Open Questions

- Should we support download resume for interrupted downloads? (Defer to future iteration)
- Should we validate downloaded file integrity via checksum? (Nice to have, not MVP)
