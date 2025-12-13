# output-format Specification

## Purpose
TBD - created by archiving change add-output-format-selection. Update Purpose after archive.
## Requirements
### Requirement: Output Format Selection

The system SHALL provide a dropdown control for selecting the output video format before recording begins.

#### Scenario: Format dropdown displayed in idle state

- **WHEN** the application is in idle state
- **THEN** an output format dropdown is visible in the controls section
- **AND** the dropdown displays the currently selected format
- **AND** the default selection is "MP4"

#### Scenario: Format options available

- **WHEN** the user opens the output format dropdown
- **THEN** the following options are displayed: MP4, WebM, MKV, QuickTime (.mov), GIF, Animated PNG, Animated WebP
- **AND** each option shows the file extension in parentheses

#### Scenario: Format selection persists

- **WHEN** the user selects a format from the dropdown
- **THEN** the selection is retained for subsequent recordings
- **AND** the dropdown displays the newly selected format

#### Scenario: Dropdown disabled during recording

- **WHEN** a recording is in progress
- **THEN** the output format dropdown is disabled
- **AND** the user cannot change the format until recording completes

### Requirement: High-Quality Source Recording

The system SHALL always record to high-quality MP4 (H.264) regardless of the selected output format.

#### Scenario: MP4 source recording created

- **WHEN** the user starts a recording with any output format selected
- **THEN** the system captures frames and encodes to MP4 using H.264 codec
- **AND** the encoding uses current quality settings (CRF 23, ultrafast preset)

#### Scenario: MP4 format selected (no transcoding)

- **WHEN** the user stops a recording with MP4 format selected
- **THEN** the recorded MP4 file is the final output
- **AND** no transcoding occurs
- **AND** only one file is created

### Requirement: Post-Recording Transcoding

The system SHALL transcode the source MP4 to the selected format when the format is not MP4.

#### Scenario: Transcoding initiated for non-MP4 format

- **WHEN** the user stops a recording with a non-MP4 format selected
- **THEN** the system first completes the MP4 encoding
- **AND** the system then initiates transcoding to the selected format

#### Scenario: Transcoding to WebM

- **WHEN** the selected format is WebM
- **THEN** the system transcodes using VP9 codec
- **AND** the output file has a `.webm` extension

#### Scenario: Transcoding to MKV

- **WHEN** the selected format is MKV
- **THEN** the system remuxes the H.264 stream to MKV container (no re-encoding)
- **AND** the output file has a `.mkv` extension

#### Scenario: Transcoding to QuickTime

- **WHEN** the selected format is QuickTime
- **THEN** the system remuxes the H.264 stream to MOV container (no re-encoding)
- **AND** the output file has a `.mov` extension

#### Scenario: Transcoding to GIF

- **WHEN** the selected format is GIF
- **THEN** the system transcodes with palette generation for quality
- **AND** the frame rate is reduced to 15 FPS for reasonable file size
- **AND** the output file has a `.gif` extension

#### Scenario: Transcoding to Animated PNG

- **WHEN** the selected format is Animated PNG
- **THEN** the system transcodes to APNG format
- **AND** the output file has a `.apng` extension

#### Scenario: Transcoding to Animated WebP

- **WHEN** the selected format is Animated WebP
- **THEN** the system transcodes using libwebp codec
- **AND** the output file has a `.webp` extension

### Requirement: Original File Retention

The system SHALL retain the original high-quality MP4 file after transcoding.

#### Scenario: Both files exist after transcoding

- **WHEN** transcoding to a non-MP4 format completes successfully
- **THEN** both the original MP4 and the transcoded file exist in the output directory
- **AND** both files have the same base name with different extensions

#### Scenario: File naming convention

- **WHEN** a recording is saved with transcoding
- **THEN** the original file is named `recording_<timestamp>.mp4`
- **AND** the transcoded file is named `recording_<timestamp>.<target_ext>`

### Requirement: Transcoding Progress Feedback

The system SHALL provide visual feedback during the transcoding phase.

#### Scenario: Transcoding status displayed

- **WHEN** transcoding is in progress
- **THEN** the status overlay displays "Transcoding to <format>..."
- **AND** the record button remains disabled

#### Scenario: Transcoding complete notification

- **WHEN** transcoding completes successfully
- **THEN** the result notification shows the transcoded file path
- **AND** the "Open Folder" button opens the folder containing both files

#### Scenario: Transcoding failure handling

- **WHEN** transcoding fails
- **THEN** an error message is displayed to the user
- **AND** the original MP4 file is preserved
- **AND** the user is informed that the MP4 recording was saved

### Requirement: Animated Format Duration Warning

The system SHALL warn users when selecting animated formats for long recordings.

#### Scenario: Long recording warning for GIF

- **WHEN** the user selects GIF format
- **AND** a previous recording was longer than 30 seconds
- **THEN** a tooltip or hint warns that GIF files can be very large for long recordings

#### Scenario: No warning for short recordings

- **WHEN** the user selects GIF, Animated PNG, or Animated WebP
- **AND** no previous recording exists or the last recording was under 30 seconds
- **THEN** no warning is displayed

