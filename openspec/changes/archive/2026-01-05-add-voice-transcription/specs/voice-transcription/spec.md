# voice-transcription Specification

## Purpose

Provides real-time voice transcription during recording, producing a timestamped markdown transcript file alongside the video output.

## ADDED Requirements

### Requirement: Whisper Model Management

The system SHALL use the whisper.cpp medium.en model for transcription.

#### Scenario: Model location

- **WHEN** transcription is enabled
- **THEN** the system SHALL look for the model at the platform-specific cache path
- **AND** on Linux the path SHALL be `~/.cache/omnirec/whisper/ggml-medium.en.bin`
- **AND** on macOS the path SHALL be `~/Library/Caches/omnirec/whisper/ggml-medium.en.bin`
- **AND** on Windows the path SHALL be `%LOCALAPPDATA%\omnirec\whisper\ggml-medium.en.bin`

#### Scenario: Model not found

- **WHEN** recording starts with transcription enabled
- **AND** the whisper model file does not exist
- **THEN** the system SHALL disable transcription for this recording
- **AND** the system SHALL log a warning with the expected model path
- **AND** the recording SHALL proceed without transcription

#### Scenario: Model loading

- **WHEN** recording starts with transcription enabled
- **AND** the model file exists
- **THEN** the system SHALL load the whisper model into memory
- **AND** the model SHALL remain loaded until the service stops or transcription is disabled

### Requirement: Voice Activity Detection

The system SHALL detect speech in the audio stream using multi-feature analysis.

#### Scenario: Speech detection features

- **WHEN** audio samples are being processed for transcription
- **THEN** the system SHALL compute RMS amplitude in decibels
- **AND** the system SHALL compute Zero-Crossing Rate (ZCR)
- **AND** the system SHALL estimate spectral centroid using first-difference approximation
- **AND** the system SHALL use these features to classify audio as speech or non-speech

#### Scenario: Voiced speech detection

- **WHEN** audio has amplitude above -42dB
- **AND** ZCR is between 0.01 and 0.30
- **AND** spectral centroid is between 200Hz and 5500Hz
- **AND** these conditions persist for 80ms
- **THEN** the system SHALL classify this as voiced speech

#### Scenario: Whisper speech detection

- **WHEN** audio has amplitude above -52dB
- **AND** ZCR is between 0.08 and 0.45
- **AND** spectral centroid is between 300Hz and 7000Hz
- **AND** these conditions persist for 120ms
- **THEN** the system SHALL classify this as whispered speech

#### Scenario: Speech end detection

- **WHEN** speech has been detected
- **AND** the audio no longer matches speech criteria
- **AND** 500ms of non-speech audio has elapsed (hold time)
- **THEN** the system SHALL classify this as speech ended

#### Scenario: Transient rejection

- **WHEN** audio has ZCR above 0.45
- **AND** spectral centroid is above 6500Hz
- **THEN** the system SHALL classify this as a transient (not speech)
- **AND** the transient SHALL NOT trigger speech detection

### Requirement: Speech Lookback

The system SHALL capture the true start of speech using a lookback buffer.

#### Scenario: Lookback buffer maintenance

- **WHEN** transcription is active
- **THEN** the system SHALL maintain a ring buffer of the last 200ms of audio
- **AND** the buffer SHALL be updated continuously regardless of speech state

#### Scenario: Speech start lookback

- **WHEN** speech is confirmed (onset threshold reached)
- **THEN** the system SHALL scan backward through the lookback buffer
- **AND** the system SHALL find where audio first exceeded -55dB (lookback threshold)
- **AND** the system SHALL include a 20ms margin before the detected start
- **AND** the segment SHALL begin from this lookback position

### Requirement: Word Break Detection

The system SHALL detect natural word boundaries during speech for segment splitting.

#### Scenario: Word break identification

- **WHEN** speech is active
- **AND** amplitude drops below 50% of recent average amplitude
- **AND** the drop persists for at least 150ms
- **AND** the drop lasts no more than 500ms
- **THEN** the system SHALL classify this as a word break

#### Scenario: Word break tracking

- **WHEN** a word break is detected
- **THEN** the system SHALL record the offset from segment start
- **AND** the system SHALL record the gap duration

### Requirement: Segment Buffering

The system SHALL buffer speech audio into segments for transcription.

#### Scenario: Ring buffer capacity

- **WHEN** transcription is active
- **THEN** the system SHALL maintain a ring buffer with 35 seconds capacity at 48kHz stereo
- **AND** the buffer SHALL support continuous write without blocking

#### Scenario: Segment start

- **WHEN** speech is detected
- **THEN** the system SHALL mark the segment start index (including lookback)
- **AND** the system SHALL begin tracking segment duration

#### Scenario: Duration threshold

- **WHEN** speech continues for 20 seconds
- **THEN** the system SHALL enter "seeking word break" mode
- **AND** the system SHALL look for a word break to split the segment

#### Scenario: Word break submission

- **WHEN** in seeking word break mode
- **AND** a word break is detected within the 2 second grace period
- **THEN** the system SHALL extract the segment up to the word break midpoint
- **AND** the system SHALL submit the segment for transcription
- **AND** the next segment SHALL begin from the submission point

#### Scenario: Grace period expiration

- **WHEN** in seeking word break mode
- **AND** 2 seconds elapse without finding a word break
- **THEN** the system SHALL extract the segment at the current position
- **AND** the system SHALL submit the segment for transcription
- **AND** the next segment SHALL begin from the submission point

#### Scenario: Maximum segment duration

- **WHEN** speech continues for 30 seconds
- **AND** no natural pause or word break has occurred
- **THEN** the system SHALL force segment extraction
- **AND** the system SHALL submit the segment for transcription
- **AND** the next segment SHALL begin from the submission point

#### Scenario: Speech end submission

- **WHEN** speech ends (500ms hold time elapsed)
- **THEN** the system SHALL extract the complete segment
- **AND** the system SHALL submit the segment for transcription

#### Scenario: Continuous audio flow

- **WHEN** a segment is submitted mid-speech
- **THEN** the next segment SHALL begin exactly where the previous ended
- **AND** no audio samples SHALL be lost between segments

### Requirement: Transcription Queue

The system SHALL process segments asynchronously using a queue.

#### Scenario: Queue capacity

- **WHEN** transcription is active
- **THEN** the system SHALL maintain a transcription queue with capacity for 10 segments
- **AND** the queue SHALL be processed by a dedicated worker thread

#### Scenario: Segment validation

- **WHEN** a segment is submitted for transcription
- **AND** the segment duration is less than 500ms
- **THEN** the segment SHALL be discarded (likely to produce blank output)

#### Scenario: Segment validation audio level

- **WHEN** a segment is submitted for transcription
- **AND** the segment RMS amplitude is below 0.01 (approximately -40dB)
- **THEN** the segment SHALL be discarded (too quiet for reliable transcription)

#### Scenario: Queue overflow

- **WHEN** a segment is submitted
- **AND** the queue is full (10 segments pending)
- **THEN** the segment SHALL be dropped
- **AND** the system SHALL log a warning

#### Scenario: Worker processing

- **WHEN** a segment is dequeued
- **THEN** the worker SHALL convert audio to 16kHz mono (whisper input format)
- **AND** the worker SHALL run whisper inference
- **AND** the worker SHALL extract the transcribed text
- **AND** the worker SHALL write the result to the transcript file

### Requirement: Whisper Configuration

The system SHALL configure whisper for longer segment transcription.

#### Scenario: Whisper parameters

- **WHEN** whisper processes a segment
- **THEN** no_context SHALL be false (use context for coherence)
- **AND** single_segment SHALL be false (allow multiple output segments)
- **AND** max_tokens SHALL be 256 (allow longer output)
- **AND** language SHALL be "en" (English only)
- **AND** translate SHALL be false (transcription only)

### Requirement: Transcript Output

The system SHALL produce a markdown transcript file.

#### Scenario: Transcript file creation

- **WHEN** recording starts with transcription enabled
- **THEN** the system SHALL create a transcript file in the output directory
- **AND** the filename SHALL be `{video_basename}_transcript.md`
- **AND** the file SHALL begin with a markdown heading `# Transcript\n\n`

#### Scenario: Segment output format

- **WHEN** a segment is transcribed successfully
- **THEN** the system SHALL append a line to the transcript file
- **AND** the line SHALL be formatted as `[HH:MM:SS] {text}`
- **AND** the timestamp SHALL be the recording elapsed time when the segment started
- **AND** the text SHALL be trimmed of leading/trailing whitespace

#### Scenario: Transcript finalization

- **WHEN** recording stops
- **THEN** the system SHALL process any remaining segments in the queue
- **AND** the system SHALL close the transcript file
- **AND** the transcript file SHALL be saved alongside the video

#### Scenario: Empty transcript handling

- **WHEN** recording stops
- **AND** no speech was detected during recording
- **THEN** the transcript file SHALL contain only the heading
- **AND** the file SHALL NOT be deleted (indicates no speech detected)

### Requirement: Audio Pipeline Integration

The system SHALL receive audio samples without affecting recording quality.

#### Scenario: Audio sample delivery

- **WHEN** transcription is enabled
- **AND** recording is active
- **THEN** the transcription module SHALL receive a copy of audio samples
- **AND** the recording pipeline SHALL NOT be blocked by transcription processing
- **AND** audio sample delivery to the encoder SHALL NOT be delayed

#### Scenario: Sample format

- **WHEN** audio samples are delivered to transcription
- **THEN** samples SHALL be 48kHz sample rate
- **AND** samples SHALL be 32-bit floating point
- **AND** samples SHALL be stereo (converted to mono for whisper)

#### Scenario: Transcription disabled

- **WHEN** transcription is disabled
- **THEN** no audio samples SHALL be processed for transcription
- **AND** no voice detection SHALL occur
- **AND** no transcript file SHALL be created

### Requirement: CUDA Acceleration

The system SHALL support GPU-accelerated transcription via compile-time feature.

#### Scenario: CUDA feature on Linux

- **WHEN** the application is built with `--features cuda` on Linux
- **THEN** whisper.cpp SHALL be built from source with `-DGGML_CUDA=ON`
- **AND** the build SHALL require NVIDIA CUDA Toolkit installed
- **AND** transcription SHALL use GPU acceleration when available

#### Scenario: CUDA feature on Windows

- **WHEN** the application is built with `--features cuda` on Windows
- **THEN** prebuilt CUDA-enabled whisper binaries SHALL be downloaded
- **AND** required CUDA runtime DLLs SHALL be bundled
- **AND** transcription SHALL use GPU acceleration when available

#### Scenario: CUDA feature on macOS

- **WHEN** the application is built with `--features cuda` on macOS
- **THEN** the feature SHALL have no effect
- **AND** Metal acceleration SHALL be used via prebuilt framework

#### Scenario: CPU fallback

- **WHEN** the application is built without the cuda feature
- **THEN** transcription SHALL use CPU-only inference
- **AND** no GPU dependencies SHALL be required

### Requirement: Service IPC Integration

The system SHALL control transcription via IPC commands.

#### Scenario: Start transcription

- **WHEN** recording starts with transcription enabled
- **THEN** the Tauri app SHALL send a start_transcription command to the service
- **AND** the command SHALL include the output file path
- **AND** the service SHALL initialize the transcription pipeline

#### Scenario: Stop transcription

- **WHEN** recording stops
- **THEN** the Tauri app SHALL send a stop_transcription command to the service
- **AND** the service SHALL finalize any pending segments
- **AND** the service SHALL close the transcript file

#### Scenario: Transcription status

- **WHEN** the Tauri app queries transcription status
- **THEN** the service SHALL return whether the model is loaded
- **AND** the service SHALL return the number of segments processed
- **AND** the service SHALL return any error state
