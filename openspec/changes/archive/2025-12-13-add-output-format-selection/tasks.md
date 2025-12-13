## 1. Backend: Output Format Types and Transcoding

- [x] 1.1 Define `OutputFormat` enum in Rust with variants: Mp4, WebM, Mkv, QuickTime, Gif, AnimatedPng, AnimatedWebp
- [x] 1.2 Add `output_format` field to application state
- [x] 1.3 Create `transcode_video()` function in encoder module that accepts source path, target format, and returns target path
- [x] 1.4 Implement FFmpeg transcoding commands for each format:
  - [x] 1.4.1 WebM (VP9): `-c:v libvpx-vp9 -crf 30 -b:v 0`
  - [x] 1.4.2 MKV (remux): `-c:v copy`
  - [x] 1.4.3 QuickTime (remux): `-c:v copy -f mov`
  - [x] 1.4.4 GIF (with palette): `-vf "fps=15,scale=640:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse"`
  - [x] 1.4.5 Animated PNG: `-plays 0 -f apng`
  - [x] 1.4.6 Animated WebP: `-c:v libwebp -lossless 0 -q:v 75 -loop 0`
- [x] 1.5 Add Tauri command `set_output_format(format: String)` to set the desired format
- [x] 1.6 Add Tauri command `get_output_format()` to retrieve current format selection

## 2. Backend: Recording Flow Integration

- [x] 2.1 Modify `stop_recording` to check selected format after MP4 encoding completes
- [x] 2.2 If format is not MP4, call `transcode_video()` with the source MP4 path
- [x] 2.3 Update `RecordingResult` struct to include both `source_path` (MP4) and `output_path` (transcoded)
- [x] 2.4 Emit Tauri event `transcoding-started` when transcoding begins
- [x] 2.5 Emit Tauri event `transcoding-complete` when transcoding finishes (success or failure)
- [x] 2.6 Handle transcoding errors gracefully, preserving the original MP4

## 3. Frontend: Format Dropdown UI

- [x] 3.1 Add `<select>` element for output format in `index.html` within the controls section
- [x] 3.2 Style the dropdown to match existing UI theme (dark gradient, rounded corners)
- [x] 3.3 Populate dropdown options: MP4, WebM, MKV, QuickTime (.mov), GIF, Animated PNG, Animated WebP
- [x] 3.4 Add TypeScript type `OutputFormat` matching backend enum
- [x] 3.5 Add state variable `selectedFormat: OutputFormat` with default "mp4"
- [x] 3.6 Wire dropdown `change` event to call `set_output_format` Tauri command
- [x] 3.7 Disable dropdown when recording state is not "idle"

## 4. Frontend: Transcoding Feedback

- [x] 4.1 Listen for `transcoding-started` event and update status overlay to "Transcoding to <format>..."
- [x] 4.2 Listen for `transcoding-complete` event and update result notification
- [x] 4.3 Update `showResult()` to display the transcoded file path (not the source MP4)
- [x] 4.4 Handle transcoding errors by showing error message while noting MP4 was saved

## 5. Testing and Validation

- [ ] 5.1 Test recording with MP4 format selected (no transcoding)
- [ ] 5.2 Test recording with WebM format selected (verify VP9 output)
- [ ] 5.3 Test recording with MKV format selected (verify fast remux)
- [ ] 5.4 Test recording with QuickTime format selected (verify .mov output)
- [ ] 5.5 Test recording with GIF format selected (verify animated output with palette)
- [ ] 5.6 Test recording with Animated PNG format selected
- [ ] 5.7 Test recording with Animated WebP format selected
- [ ] 5.8 Verify original MP4 is retained after transcoding
- [ ] 5.9 Verify dropdown is disabled during recording
- [ ] 5.10 Test transcoding failure scenario (ensure MP4 is preserved)
