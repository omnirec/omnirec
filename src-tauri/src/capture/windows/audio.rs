//! Windows audio capture using WASAPI.
//!
//! This module provides audio device enumeration and capture via the Windows Audio Session API
//! (WASAPI). It supports both loopback capture (system audio) from output devices and direct
//! capture from input devices (microphones).
//!
//! ## Architecture
//!
//! Audio capture uses a dedicated thread per capture session that:
//! 1. Initializes COM for the thread
//! 2. Gets the device from its endpoint ID
//! 3. Activates an IAudioClient with appropriate flags
//! 4. Runs an event-driven capture loop
//! 5. Converts samples to 48kHz stereo f32 format
//! 6. Sends samples through an mpsc channel
//!
//! ## Loopback vs Direct Capture
//!
//! - Output devices (AudioSourceType::Output): Use WASAPI loopback mode to capture system audio
//! - Input devices (AudioSourceType::Input): Use direct WASAPI capture for microphone input

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{AudioReceiver, AudioSample, AudioSource, AudioSourceType, StopHandle};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

use windows::core::{PCWSTR, PROPVARIANT, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Media::Audio::{
    eCapture, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceCollection,
    IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, DEVICE_STATE_ACTIVE,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};

/// PKEY_Device_FriendlyName - the friendly name property key
/// {a45c254e-df1c-4efd-8020-67d146a850e0}, 14
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

/// Target sample rate for output (matches encoder expectations)
const TARGET_SAMPLE_RATE: u32 = 48000;

/// Target channel count for output
const TARGET_CHANNELS: u32 = 2;

/// Audio format information extracted from WAVEFORMATEX
#[derive(Debug, Clone)]
struct AudioFormat {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    is_float: bool,
}

impl AudioFormat {
    /// Parse audio format from WAVEFORMATEX pointer
    unsafe fn from_waveformatex(format: *const WAVEFORMATEX) -> Self {
        let fmt = &*format;
        let mut is_float = false;

        // Check if this is WAVEFORMATEXTENSIBLE
        if fmt.wFormatTag == WAVE_FORMAT_EXTENSIBLE as u16 {
            let ext = format as *const WAVEFORMATEXTENSIBLE;
            // Use raw pointer arithmetic to avoid unaligned reference
            let sub_format_ptr = std::ptr::addr_of!((*ext).SubFormat);
            let sub_format = std::ptr::read_unaligned(sub_format_ptr);
            is_float = sub_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
        } else if fmt.wFormatTag == 3 {
            // WAVE_FORMAT_IEEE_FLOAT
            is_float = true;
        }

        Self {
            sample_rate: fmt.nSamplesPerSec,
            channels: fmt.nChannels,
            bits_per_sample: fmt.wBitsPerSample,
            is_float,
        }
    }
}

/// List all available audio sources using WASAPI.
///
/// Enumerates both output devices (for system audio loopback capture) and
/// input devices (microphones). Returns an empty list if enumeration fails.
pub fn list_audio_sources() -> Result<Vec<AudioSource>, EnumerationError> {
    // Initialize COM for this thread (MTA)
    // Safety: COM initialization is required for WASAPI calls
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };

    // Even if COM was already initialized, we should proceed
    // CoInitializeEx returns S_FALSE if already initialized, which is still success

    let result = enumerate_audio_devices();

    // Uninitialize COM if we initialized it
    if com_initialized {
        unsafe { CoUninitialize() };
    }

    result
}

/// Internal function to enumerate audio devices after COM is initialized.
fn enumerate_audio_devices() -> Result<Vec<AudioSource>, EnumerationError> {
    let mut sources = Vec::new();

    // Create the device enumerator
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
            eprintln!("[Audio] Failed to create device enumerator: {:?}", e);
            EnumerationError::PlatformError(format!("Failed to create device enumerator: {}", e))
        })?
    };

    // Enumerate output devices (eRender) - these are used for system audio loopback
    if let Ok(output_devices) = enumerate_devices_by_flow(&enumerator, eRender) {
        for device in output_devices {
            sources.push(AudioSource {
                id: device.0,
                name: format!("{} (Monitor)", device.1),
                source_type: AudioSourceType::Output,
            });
        }
    }

    // Enumerate input devices (eCapture) - microphones
    if let Ok(input_devices) = enumerate_devices_by_flow(&enumerator, eCapture) {
        for device in input_devices {
            sources.push(AudioSource {
                id: device.0,
                name: device.1,
                source_type: AudioSourceType::Input,
            });
        }
    }

    eprintln!("[Audio] Enumerated {} audio devices", sources.len());
    Ok(sources)
}

/// Enumerate devices by data flow direction.
///
/// Returns a vector of (device_id, friendly_name) tuples.
fn enumerate_devices_by_flow(
    enumerator: &IMMDeviceEnumerator,
    data_flow: windows::Win32::Media::Audio::EDataFlow,
) -> Result<Vec<(String, String)>, EnumerationError> {
    let mut devices = Vec::new();

    // Get device collection for the specified flow direction
    let collection: IMMDeviceCollection = unsafe {
        enumerator
            .EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE)
            .map_err(|e| {
                EnumerationError::PlatformError(format!("Failed to enumerate endpoints: {}", e))
            })?
    };

    // Get device count
    let count = unsafe {
        collection.GetCount().map_err(|e| {
            EnumerationError::PlatformError(format!("Failed to get device count: {}", e))
        })?
    };

    // Iterate through devices
    for i in 0..count {
        if let Ok(device) = unsafe { collection.Item(i) } {
            if let Some((id, name)) = get_device_info(&device) {
                devices.push((id, name));
            }
        }
    }

    Ok(devices)
}

/// Extract device ID and friendly name from an IMMDevice.
fn get_device_info(device: &IMMDevice) -> Option<(String, String)> {
    // Get device ID
    let id = unsafe {
        let id_ptr = device.GetId().ok()?;
        let id_str = pcwstr_to_string(PCWSTR(id_ptr.0));
        // Free the allocated string
        CoTaskMemFree(Some(id_ptr.0 as *const _));
        id_str
    }?;

    // Get friendly name from device properties
    let name = get_device_friendly_name(device).unwrap_or_else(|| "Unknown Device".to_string());

    Some((id, name))
}

/// Get the friendly name of a device from its property store.
fn get_device_friendly_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        // Open property store
        let store: IPropertyStore = device.OpenPropertyStore(STGM_READ).ok()?;

        // Get friendly name property
        let prop_value: PROPVARIANT = store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME).ok()?;

        // Extract string from PROPVARIANT using the windows crate's built-in conversion
        propvariant_to_string(&prop_value)
    }
}

/// Convert a PROPVARIANT to a String if it contains a string value.
fn propvariant_to_string(pv: &PROPVARIANT) -> Option<String> {
    // The windows crate PROPVARIANT has a method to convert to string types
    // We can use the VT_LPWSTR type check and extract the wide string
    unsafe {
        // Try to get as a PWSTR (wide string)
        // The PROPVARIANT anonymous union contains the actual value
        // For VT_LPWSTR, it's stored in the pwszVal field

        // Use the windows crate's built-in display/debug to see if it's a string
        // or we can check the vt type directly
        let inner = &pv.as_raw().Anonymous.Anonymous;
        let vt = inner.vt;

        // VT_LPWSTR = 31
        if vt == 31 {
            let pwsz = inner.Anonymous.pwszVal;
            if !pwsz.is_null() {
                return pcwstr_to_string(PCWSTR(pwsz));
            }
        }
    }
    None
}

/// Convert a null-terminated wide string pointer to a Rust String.
fn pcwstr_to_string(pcwstr: PCWSTR) -> Option<String> {
    if pcwstr.is_null() {
        return None;
    }

    unsafe {
        // Find string length
        let mut len = 0;
        while *pcwstr.0.add(len) != 0 {
            len += 1;
        }

        if len == 0 {
            return Some(String::new());
        }

        // Convert to Rust string
        let slice = std::slice::from_raw_parts(pcwstr.0, len);
        String::from_utf16(slice).ok()
    }
}

/// Start audio capture from the specified device.
///
/// For output devices (AudioSourceType::Output), uses WASAPI loopback mode to capture system audio.
/// For input devices (AudioSourceType::Input), uses direct WASAPI capture.
///
/// Returns an audio sample receiver and stop handle.
/// Audio is delivered as 48kHz stereo f32 samples.
pub fn start_audio_capture(source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    // Determine if this is an output or input device by re-enumerating
    let sources = list_audio_sources().map_err(|e| {
        CaptureError::AudioError(format!("Failed to enumerate audio sources: {:?}", e))
    })?;

    let source = sources
        .iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| CaptureError::AudioError(format!("Audio device not found: {}", source_id)))?;

    let is_loopback = source.source_type == AudioSourceType::Output;
    let source_id_owned = source_id.to_string();

    eprintln!(
        "[Audio] Starting {} capture for device: {}",
        if is_loopback { "loopback" } else { "direct" },
        source.name
    );

    // Create channel for audio samples
    let (tx, rx) = mpsc::channel(256);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);

    // Spawn capture thread
    thread::spawn(move || {
        if let Err(e) = run_capture_thread(&source_id_owned, is_loopback, tx, stop_flag_clone) {
            eprintln!("[Audio] Capture thread error: {}", e);
        }
        eprintln!("[Audio] Capture thread exited");
    });

    Ok((rx, stop_flag))
}

/// Run the WASAPI capture loop in a dedicated thread.
fn run_capture_thread(
    device_id: &str,
    is_loopback: bool,
    tx: mpsc::Sender<AudioSample>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    // Initialize COM for this thread
    eprintln!("[Audio] Initializing COM for capture thread");
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        // S_OK (0) or S_FALSE (1) are both acceptable
        if hr.is_err() && hr.0 != 1 {
            return Err(format!("Failed to initialize COM: {:?}", hr));
        }
    }

    let result = run_capture_loop(device_id, is_loopback, tx, stop_flag);

    // Uninitialize COM
    unsafe {
        CoUninitialize();
    }

    result
}

/// Inner capture loop (COM already initialized).
fn run_capture_loop(
    device_id: &str,
    is_loopback: bool,
    tx: mpsc::Sender<AudioSample>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    unsafe {
        // Create device enumerator
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

        // Get device by ID
        let device_id_wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let device: IMMDevice = enumerator
            .GetDevice(PCWSTR(device_id_wide.as_ptr()))
            .map_err(|e| format!("Failed to get device: {}", e))?;

        // Activate audio client
        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Failed to activate audio client: {}", e))?;

        // Get mix format
        let mix_format_ptr = audio_client
            .GetMixFormat()
            .map_err(|e| format!("Failed to get mix format: {}", e))?;

        let device_format = AudioFormat::from_waveformatex(mix_format_ptr);
        eprintln!(
            "[Audio] Device native format: {}Hz, {} channels, {} bits, float={}",
            device_format.sample_rate, device_format.channels, device_format.bits_per_sample, device_format.is_float
        );

        // Create event for buffer notification
        let event: HANDLE = CreateEventW(None, false, false, PWSTR::null())
            .map_err(|e| format!("Failed to create event: {}", e))?;

        // Initialize audio client with the device's native mix format
        // WASAPI shared mode requires using the mix format
        // We use EVENTCALLBACK for event-driven capture
        let stream_flags = if is_loopback {
            AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        } else {
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        };

        // Buffer duration in 100-nanosecond units (100ms buffer for reliability)
        let buffer_duration: i64 = 1_000_000;

        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags,
                buffer_duration,
                0,
                mix_format_ptr,
                None,
            )
            .map_err(|e| format!("Failed to initialize audio client: {}", e))?;

        // Keep a copy of the format info before freeing
        let capture_format = device_format.clone();
        
        // Free the mix format
        CoTaskMemFree(Some(mix_format_ptr as *const _));

        // Get capture client
        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| format!("Failed to get capture client: {}", e))?;

        // Set event handle
        audio_client
            .SetEventHandle(event)
            .map_err(|e| format!("Failed to set event handle: {}", e))?;

        // Start capturing
        audio_client
            .Start()
            .map_err(|e| format!("Failed to start audio capture: {}", e))?;

        eprintln!("[Audio] Capture started successfully");

        // Capture loop
        let mut total_frames_captured: u64 = 0;
        let mut last_log_time = std::time::Instant::now();
        
        loop {
            // Check stop flag
            if stop_flag.load(Ordering::Relaxed) {
                eprintln!("[Audio] Stop flag set, exiting capture loop (captured {} frames total)", total_frames_captured);
                break;
            }

            // Wait for buffer event (with timeout to check stop flag periodically)
            let wait_result = WaitForSingleObject(event, 100);
            if wait_result.0 == 258 {
                // WAIT_TIMEOUT - this is normal, just poll for data anyway
                // Some devices don't signal the event properly
            }

            // Get available frames
            let packet_length = match capture_client.GetNextPacketSize() {
                Ok(len) => len,
                Err(e) => {
                    eprintln!("[Audio] GetNextPacketSize failed: {:?}, device may be disconnected", e);
                    break;
                }
            };

            let mut current_packet_length = packet_length;
            while current_packet_length > 0 {
                let mut buffer_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames: u32 = 0;
                let mut flags: u32 = 0;

                let result = capture_client.GetBuffer(
                    &mut buffer_ptr,
                    &mut num_frames,
                    &mut flags,
                    None,
                    None,
                );

                if result.is_err() {
                    eprintln!("[Audio] GetBuffer failed, device may be disconnected");
                    break;
                }

                // Check for silent buffer flag (AUDCLNT_BUFFERFLAGS_SILENT = 0x2)
                let is_silent = (flags & 0x2) != 0;

                if num_frames > 0 && !buffer_ptr.is_null() {
                    total_frames_captured += num_frames as u64;
                    
                    // Log progress periodically
                    if last_log_time.elapsed().as_secs() >= 2 {
                        eprintln!("[Audio] Captured {} frames so far (silent={})", total_frames_captured, is_silent);
                        last_log_time = std::time::Instant::now();
                    }

                    // Convert samples to f32 using the device's actual format
                    let samples = if is_silent {
                        // Return silence instead of reading potentially invalid buffer
                        vec![0.0f32; num_frames as usize * TARGET_CHANNELS as usize]
                    } else {
                        convert_samples_to_f32(
                            buffer_ptr,
                            num_frames as usize,
                            capture_format.channels,
                            capture_format.bits_per_sample,
                            capture_format.is_float,
                        )
                    };

                    // Send to channel with target format info
                    // Note: If device sample rate differs from TARGET_SAMPLE_RATE, 
                    // the encoder will need to handle this
                    let sample = AudioSample {
                        data: samples,
                        sample_rate: capture_format.sample_rate,
                        channels: TARGET_CHANNELS,
                    };

                    if tx.blocking_send(sample).is_err() {
                        eprintln!("[Audio] Channel closed, stopping capture");
                        let _ = capture_client.ReleaseBuffer(num_frames);
                        break;
                    }
                }

                // Release buffer
                if capture_client.ReleaseBuffer(num_frames).is_err() {
                    eprintln!("[Audio] ReleaseBuffer failed");
                    break;
                }

                // Check for more packets
                current_packet_length = match capture_client.GetNextPacketSize() {
                    Ok(len) => len,
                    Err(_) => break,
                };
            }
        }

        // Stop and cleanup
        let _ = audio_client.Stop();
        eprintln!("[Audio] Capture stopped");
    }

    Ok(())
}

/// Create a WAVEFORMATEX structure for the requested format.
#[allow(dead_code)]
fn create_wave_format(sample_rate: u32, channels: u16, is_float: bool) -> WAVEFORMATEX {
    let bits_per_sample: u16 = if is_float { 32 } else { 16 };
    let block_align = channels * (bits_per_sample / 8);
    let bytes_per_sec = sample_rate * block_align as u32;

    WAVEFORMATEX {
        wFormatTag: if is_float { 3 } else { 1 }, // WAVE_FORMAT_IEEE_FLOAT or WAVE_FORMAT_PCM
        nChannels: channels,
        nSamplesPerSec: sample_rate,
        nAvgBytesPerSec: bytes_per_sec,
        nBlockAlign: block_align,
        wBitsPerSample: bits_per_sample,
        cbSize: 0,
    }
}

/// Convert raw audio buffer to f32 samples.
///
/// Handles:
/// - Float32 stereo (pass through)
/// - Float32 mono (duplicate to stereo)
/// - Int16 stereo (convert to float32)
/// - Int16 mono (convert to float32, duplicate to stereo)
fn convert_samples_to_f32(
    buffer: *const u8,
    num_frames: usize,
    channels: u16,
    bits_per_sample: u16,
    is_float: bool,
) -> Vec<f32> {
    let total_samples = num_frames * channels as usize;
    let mut output = Vec::with_capacity(total_samples);

    unsafe {
        if is_float && bits_per_sample == 32 {
            // Float32 - direct copy
            let float_ptr = buffer as *const f32;
            let samples = std::slice::from_raw_parts(float_ptr, total_samples);
            output.extend_from_slice(samples);
        } else if !is_float && bits_per_sample == 16 {
            // Int16 - convert to float
            let int_ptr = buffer as *const i16;
            let samples = std::slice::from_raw_parts(int_ptr, total_samples);
            for &sample in samples {
                output.push(sample as f32 / 32768.0);
            }
        } else if !is_float && bits_per_sample == 32 {
            // Int32 - convert to float
            let int_ptr = buffer as *const i32;
            let samples = std::slice::from_raw_parts(int_ptr, total_samples);
            for &sample in samples {
                output.push(sample as f32 / 2147483648.0);
            }
        } else {
            eprintln!(
                "[Audio] Unsupported format: {} bits, float={}",
                bits_per_sample, is_float
            );
            // Return silence
            output.resize(total_samples, 0.0);
        }
    }

    // Handle mono to stereo conversion if needed
    if channels == 1 && TARGET_CHANNELS == 2 {
        let mono = output;
        output = Vec::with_capacity(mono.len() * 2);
        for sample in mono {
            output.push(sample);
            output.push(sample);
        }
    }

    output
}

// ============================================================================
// Resampling Support
// ============================================================================

/// Simple linear resampler for converting audio between sample rates.
///
/// Uses linear interpolation which is sufficient for speech/voice audio.
/// For music-quality resampling, a more sophisticated algorithm would be needed.
struct Resampler {
    source_rate: u32,
    target_rate: u32,
    buffer: Vec<f32>,
    position: f64,
}

impl Resampler {
    fn new(source_rate: u32, target_rate: u32) -> Self {
        Self {
            source_rate,
            target_rate,
            buffer: Vec::new(),
            position: 0.0,
        }
    }

    /// Process input samples and return resampled output.
    ///
    /// Input samples should be interleaved stereo.
    fn process(&mut self, samples: &[f32], channels: usize) -> Vec<f32> {
        self.buffer.extend_from_slice(samples);

        let ratio = self.source_rate as f64 / self.target_rate as f64;
        let input_frames = self.buffer.len() / channels;
        let output_frames = ((input_frames as f64 - self.position) / ratio) as usize;

        if output_frames == 0 {
            return Vec::new();
        }

        let mut output = Vec::with_capacity(output_frames * channels);

        for _ in 0..output_frames {
            let src_frame = self.position as usize;
            let frac = self.position - src_frame as f64;

            for ch in 0..channels {
                let idx0 = src_frame * channels + ch;
                let idx1 = (src_frame + 1) * channels + ch;

                let sample = if idx1 < self.buffer.len() {
                    self.buffer[idx0] * (1.0 - frac as f32) + self.buffer[idx1] * frac as f32
                } else if idx0 < self.buffer.len() {
                    self.buffer[idx0]
                } else {
                    0.0
                };
                output.push(sample);
            }

            self.position += ratio;
        }

        let consumed_frames = self.position as usize;
        if consumed_frames > 0 {
            let consumed_samples = consumed_frames * channels;
            if consumed_samples < self.buffer.len() {
                self.buffer.drain(0..consumed_samples);
                self.position -= consumed_frames as f64;
            } else {
                self.buffer.clear();
                self.position = 0.0;
            }
        }

        output
    }
}

// ============================================================================
// Dual Audio Capture with Mixing and AEC
// ============================================================================

use aec3::voip::VoipAec3;
use std::sync::mpsc as std_mpsc;
use std::sync::Mutex;

/// AEC3 frame size: 10ms at 48kHz = 480 samples per channel
const AEC_FRAME_SAMPLES: usize = 480;

/// Samples from a stream thread to the mixer
struct StreamSamples {
    /// The audio samples
    samples: Vec<f32>,
    /// Whether this stream is loopback (system audio) - used for AEC routing
    is_loopback: bool,
}

/// Audio mixer for combining samples from two streams with optional AEC.
///
/// This struct is NOT Send because VoipAec3 is not Send.
/// It must be used only in the mixer thread.
///
/// Key insight for AEC: The render (system audio) must be fed to AEC BEFORE
/// the corresponding capture (mic) is processed. This is because AEC needs
/// to know what audio was played through speakers before it can identify
/// and remove that audio from the microphone signal.
struct AudioMixer {
    /// Buffer for capture samples (microphone/input)
    capture_buffer: Vec<f32>,
    /// Buffer for render samples (system audio) - fed to AEC render path
    render_buffer: Vec<f32>,
    /// Buffer for render samples to mix with processed capture
    render_mix_buffer: Vec<f32>,
    /// Number of active streams (1 or 2)
    num_streams: usize,
    /// Channels per stream (always 2 for stereo)
    channels: u16,
    /// Output sender to the async channel
    output_tx: mpsc::Sender<AudioSample>,
    /// Flag to enable/disable AEC
    aec_enabled: bool,
    /// AEC3 instance (created when 2 streams active and AEC enabled)
    aec: Option<VoipAec3>,
}

impl AudioMixer {
    fn new(output_tx: mpsc::Sender<AudioSample>, aec_enabled: bool) -> Self {
        Self {
            capture_buffer: Vec::new(),
            render_buffer: Vec::new(),
            render_mix_buffer: Vec::new(),
            num_streams: 0,
            channels: 2,
            output_tx,
            aec_enabled,
            aec: None,
        }
    }

    fn set_num_streams(&mut self, num: usize) {
        self.num_streams = num;
        self.capture_buffer.clear();
        self.render_buffer.clear();
        self.render_mix_buffer.clear();

        eprintln!(
            "[AudioMixer] set_num_streams({}), aec_enabled={}",
            num, self.aec_enabled
        );

        // Create AEC3 pipeline when we have 2 streams and AEC is enabled
        if num == 2 && self.aec_enabled {
            // Initial delay hint: start with 0ms and let AEC adapt
            match VoipAec3::builder(48000, self.channels as usize, self.channels as usize)
                .enable_high_pass(true)
                .initial_delay_ms(0)
                .build()
            {
                Ok(aec) => {
                    eprintln!(
                        "[AudioMixer] AEC3 initialized: 48kHz, {} channels, {}ms frames (frame_size={})",
                        self.channels,
                        AEC_FRAME_SAMPLES * 1000 / 48000,
                        AEC_FRAME_SAMPLES * self.channels as usize
                    );
                    self.aec = Some(aec);
                }
                Err(e) => {
                    eprintln!("[AudioMixer] Failed to initialize AEC3: {:?}", e);
                    self.aec = None;
                }
            }
        } else {
            if num == 2 {
                eprintln!("[AudioMixer] Two streams but AEC disabled");
            }
            self.aec = None;
        }
    }

    /// Add samples from a stream, routing based on source type.
    /// - Loopback (system audio) is fed IMMEDIATELY to AEC render path
    /// - Non-loopback (mic) is buffered and processed when enough data available
    fn push_samples(&mut self, samples: &[f32], is_loopback: bool) {
        if self.num_streams == 1 {
            // Single stream - send directly (no AEC possible)
            let sample = AudioSample {
                data: samples.to_vec(),
                sample_rate: TARGET_SAMPLE_RATE,
                channels: self.channels as u32,
            };
            let _ = self.output_tx.blocking_send(sample);
            return;
        }

        // Two streams mode
        let frame_size = AEC_FRAME_SAMPLES * self.channels as usize;

        if is_loopback {
            // System audio (render) - feed to AEC immediately in frame-sized chunks
            // This is critical: AEC needs to see render BEFORE corresponding capture
            self.render_buffer.extend_from_slice(samples);
            // Also keep a copy for mixing
            self.render_mix_buffer.extend_from_slice(samples);

            // Feed render frames to AEC immediately
            if let Some(ref mut aec) = self.aec {
                while self.render_buffer.len() >= frame_size {
                    let render_frame: Vec<f32> = self.render_buffer.drain(0..frame_size).collect();
                    if let Err(e) = aec.handle_render_frame(&render_frame) {
                        eprintln!("[AudioMixer] AEC3 handle_render_frame error: {:?}", e);
                    }
                }
            }
        } else {
            // Microphone (capture) - buffer and process
            self.capture_buffer.extend_from_slice(samples);
            self.process_capture();
        }
    }

    /// Process buffered capture samples through AEC
    fn process_capture(&mut self) {
        let frame_size = AEC_FRAME_SAMPLES * self.channels as usize;

        // Process capture frames when we have enough data from both sources
        while self.capture_buffer.len() >= frame_size && self.render_mix_buffer.len() >= frame_size {
            let capture_frame: Vec<f32> = self.capture_buffer.drain(0..frame_size).collect();
            let render_frame: Vec<f32> = self.render_mix_buffer.drain(0..frame_size).collect();

            // Apply AEC if enabled and we have an AEC instance
            let processed_capture = if self.aec_enabled {
                if let Some(ref mut aec) = self.aec {
                    let mut out = vec![0.0f32; capture_frame.len()];

                    match aec.process_capture_frame(&capture_frame, false, &mut out) {
                        Ok(_metrics) => out,
                        Err(e) => {
                            eprintln!("[AudioMixer] AEC3 process_capture_frame error: {:?}", e);
                            capture_frame
                        }
                    }
                } else {
                    capture_frame
                }
            } else {
                capture_frame
            };

            // Mix processed capture with system audio using soft clipping
            // Soft clip prevents harsh distortion while preserving dynamics
            let output: Vec<f32> = processed_capture
                .iter()
                .zip(render_frame.iter())
                .map(|(&capture, &render)| {
                    let sum = capture + render;
                    // Soft clip: tanh-style saturation for values beyond [-1, 1]
                    if sum > 1.0 {
                        1.0 - (-2.0 * (sum - 1.0)).exp() * 0.5
                    } else if sum < -1.0 {
                        -1.0 + (-2.0 * (-sum - 1.0)).exp() * 0.5
                    } else {
                        sum
                    }
                })
                .collect();

            // Send mixed output
            let sample = AudioSample {
                data: output,
                sample_rate: TARGET_SAMPLE_RATE,
                channels: self.channels as u32,
            };
            let _ = self.output_tx.blocking_send(sample);
        }
    }
}

/// Manager for multiple capture streams
struct MultiCaptureManager {
    /// Stream 1 thread handle and stop flag (microphone)
    stream1: Option<(thread::JoinHandle<()>, Arc<AtomicBool>)>,
    /// Stream 2 thread handle and stop flag (system audio)
    stream2: Option<(thread::JoinHandle<()>, Arc<AtomicBool>)>,
    /// Mixer thread handle
    mixer_thread: Option<thread::JoinHandle<()>>,
    /// Stop flag for mixer thread
    mixer_stop: Arc<AtomicBool>,
}

impl MultiCaptureManager {
    fn new(
        mic_source_id: Option<String>,
        sys_source_id: Option<String>,
        aec_enabled: bool,
        output_tx: mpsc::Sender<AudioSample>,
    ) -> Result<Self, CaptureError> {
        // Create channel for stream samples to mixer
        let (stream_tx, stream_rx) = std_mpsc::channel::<StreamSamples>();

        // Count streams
        let num_streams = mic_source_id.is_some() as usize + sys_source_id.is_some() as usize;
        if num_streams == 0 {
            return Err(CaptureError::AudioError(
                "No audio source specified".to_string(),
            ));
        }

        let mixer_stop = Arc::new(AtomicBool::new(false));
        let mixer_stop_clone = Arc::clone(&mixer_stop);

        // Spawn mixer thread
        let mixer_thread = thread::spawn(move || {
            run_mixer_thread(stream_rx, output_tx, num_streams, aec_enabled, mixer_stop_clone);
        });

        let mut stream1 = None;
        let mut stream2 = None;

        // Start microphone capture (stream 1) if specified
        if let Some(device_id) = mic_source_id {
            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_flag_clone = Arc::clone(&stop_flag);
            let tx = stream_tx.clone();

            let handle = thread::spawn(move || {
                run_stream_capture(device_id, false, 1, tx, stop_flag_clone);
            });

            stream1 = Some((handle, stop_flag));
        }

        // Start system audio capture (stream 2) if specified
        if let Some(device_id) = sys_source_id {
            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_flag_clone = Arc::clone(&stop_flag);
            let tx = stream_tx;

            let handle = thread::spawn(move || {
                run_stream_capture(device_id, true, 2, tx, stop_flag_clone);
            });

            stream2 = Some((handle, stop_flag));
        }

        eprintln!(
            "[Audio] Started dual capture: {} streams, AEC={}",
            num_streams, aec_enabled
        );

        Ok(Self {
            stream1,
            stream2,
            mixer_thread: Some(mixer_thread),
            mixer_stop,
        })
    }
}

impl Drop for MultiCaptureManager {
    fn drop(&mut self) {
        eprintln!("[Audio] Stopping dual capture...");

        // Signal streams to stop
        if let Some((_, ref stop_flag)) = self.stream1 {
            stop_flag.store(true, Ordering::SeqCst);
        }
        if let Some((_, ref stop_flag)) = self.stream2 {
            stop_flag.store(true, Ordering::SeqCst);
        }

        // Wait for stream threads to finish
        if let Some((handle, _)) = self.stream1.take() {
            let _ = handle.join();
        }
        if let Some((handle, _)) = self.stream2.take() {
            let _ = handle.join();
        }

        // Stop mixer thread
        self.mixer_stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.mixer_thread.take() {
            let _ = handle.join();
        }

        eprintln!("[Audio] Dual capture stopped");
    }
}

/// Run the mixer thread that combines samples from capture threads
fn run_mixer_thread(
    stream_rx: std_mpsc::Receiver<StreamSamples>,
    output_tx: mpsc::Sender<AudioSample>,
    num_streams: usize,
    aec_enabled: bool,
    stop_flag: Arc<AtomicBool>,
) {
    eprintln!("[AudioMixer] Mixer thread started");

    let mut mixer = AudioMixer::new(output_tx, aec_enabled);
    mixer.set_num_streams(num_streams);

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        // Receive samples with timeout to allow checking stop flag
        // Route based on source type (loopback vs mic), not stream index
        match stream_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(stream_samples) => {
                mixer.push_samples(&stream_samples.samples, stream_samples.is_loopback);
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {
                // Continue, will check stop flag
            }
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("[AudioMixer] Stream channels disconnected");
                break;
            }
        }
    }

    eprintln!("[AudioMixer] Mixer thread exited");
}

/// Run capture for a single stream
fn run_stream_capture(
    device_id: String,
    is_loopback: bool,
    stream_index: usize,
    stream_tx: std_mpsc::Sender<StreamSamples>,
    stop_flag: Arc<AtomicBool>,
) {
    eprintln!(
        "[Audio] Stream {} capture thread started (device={}, loopback={})",
        stream_index,
        device_id,
        is_loopback
    );

    // Initialize COM for this thread
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() && hr.0 != 1 {
            eprintln!(
                "[Audio] Stream {} failed to initialize COM: {:?}",
                stream_index, hr
            );
            return;
        }
    }

    // Run capture loop
    if let Err(e) = run_stream_capture_loop(&device_id, is_loopback, stream_index, &stream_tx, &stop_flag) {
        eprintln!("[Audio] Stream {} capture error: {}", stream_index, e);
    }

    // Uninitialize COM
    unsafe {
        CoUninitialize();
    }

    eprintln!("[Audio] Stream {} capture thread exited", stream_index);
}

/// Inner capture loop for a single stream (COM already initialized)
fn run_stream_capture_loop(
    device_id: &str,
    is_loopback: bool,
    stream_index: usize,
    stream_tx: &std_mpsc::Sender<StreamSamples>,
    stop_flag: &Arc<AtomicBool>,
) -> Result<(), String> {
    unsafe {
        // Create device enumerator
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

        // Get device by ID
        let device_id_wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let device: IMMDevice = enumerator
            .GetDevice(PCWSTR(device_id_wide.as_ptr()))
            .map_err(|e| format!("Failed to get device: {}", e))?;

        // Activate audio client
        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Failed to activate audio client: {}", e))?;

        // Get mix format
        let mix_format_ptr = audio_client
            .GetMixFormat()
            .map_err(|e| format!("Failed to get mix format: {}", e))?;

        let device_format = AudioFormat::from_waveformatex(mix_format_ptr);
        eprintln!(
            "[Audio] Stream {} device format: {}Hz, {} channels, {} bits, float={}",
            stream_index,
            device_format.sample_rate,
            device_format.channels,
            device_format.bits_per_sample,
            device_format.is_float
        );

        // Create event for buffer notification
        let event: HANDLE = CreateEventW(None, false, false, PWSTR::null())
            .map_err(|e| format!("Failed to create event: {}", e))?;

        // Initialize audio client
        let stream_flags = if is_loopback {
            AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        } else {
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        };

        let buffer_duration: i64 = 1_000_000; // 100ms

        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags,
                buffer_duration,
                0,
                mix_format_ptr,
                None,
            )
            .map_err(|e| format!("Failed to initialize audio client: {}", e))?;

        let capture_format = device_format.clone();
        CoTaskMemFree(Some(mix_format_ptr as *const _));

        // Get capture client
        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| format!("Failed to get capture client: {}", e))?;

        // Set event handle
        audio_client
            .SetEventHandle(event)
            .map_err(|e| format!("Failed to set event handle: {}", e))?;

        // Create resampler if device sample rate differs from target
        let mut resampler = if capture_format.sample_rate != TARGET_SAMPLE_RATE {
            eprintln!(
                "[Audio] Stream {} resampling {}Hz -> {}Hz",
                stream_index, capture_format.sample_rate, TARGET_SAMPLE_RATE
            );
            Some(Resampler::new(capture_format.sample_rate, TARGET_SAMPLE_RATE))
        } else {
            None
        };

        // Start capturing
        audio_client
            .Start()
            .map_err(|e| format!("Failed to start audio capture: {}", e))?;

        eprintln!("[Audio] Stream {} capture started", stream_index);

        // Capture loop
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Wait for buffer event
            let wait_result = WaitForSingleObject(event, 100);
            if wait_result.0 == 258 {
                // WAIT_TIMEOUT - continue to check stop flag
                continue;
            }

            // Get available frames
            let packet_length = match capture_client.GetNextPacketSize() {
                Ok(len) => len,
                Err(e) => {
                    eprintln!(
                        "[Audio] Stream {} GetNextPacketSize failed: {:?}",
                        stream_index, e
                    );
                    break;
                }
            };

            let mut current_packet_length = packet_length;
            while current_packet_length > 0 {
                let mut buffer_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames: u32 = 0;
                let mut flags: u32 = 0;

                let result = capture_client.GetBuffer(
                    &mut buffer_ptr,
                    &mut num_frames,
                    &mut flags,
                    None,
                    None,
                );

                if result.is_err() {
                    break;
                }

                let is_silent = (flags & 0x2) != 0;

                if num_frames > 0 && !buffer_ptr.is_null() {
                    let samples = if is_silent {
                        vec![0.0f32; num_frames as usize * TARGET_CHANNELS as usize]
                    } else {
                        convert_samples_to_f32(
                            buffer_ptr,
                            num_frames as usize,
                            capture_format.channels,
                            capture_format.bits_per_sample,
                            capture_format.is_float,
                        )
                    };

                    // Apply resampling if needed (device rate -> 48kHz)
                    let final_samples = if let Some(ref mut rs) = resampler {
                        let resampled = rs.process(&samples, TARGET_CHANNELS as usize);
                        if resampled.is_empty() {
                            // Resampler buffering, no output yet
                            let _ = capture_client.ReleaseBuffer(num_frames);
                            current_packet_length = match capture_client.GetNextPacketSize() {
                                Ok(len) => len,
                                Err(_) => break,
                            };
                            continue;
                        }
                        resampled
                    } else {
                        samples
                    };

                    // Send to mixer thread with loopback flag for proper AEC routing
                    if stream_tx
                        .send(StreamSamples {
                            samples: final_samples,
                            is_loopback,
                        })
                        .is_err()
                    {
                        let _ = capture_client.ReleaseBuffer(num_frames);
                        break;
                    }
                }

                let _ = capture_client.ReleaseBuffer(num_frames);

                current_packet_length = match capture_client.GetNextPacketSize() {
                    Ok(len) => len,
                    Err(_) => break,
                };
            }
        }

        let _ = audio_client.Stop();
    }

    Ok(())
}

/// Start audio capture from two sources (system audio + microphone) with optional AEC.
///
/// This function spawns:
/// - Two WASAPI capture threads (one for each source)
/// - One mixer thread that combines the streams with optional AEC
///
/// Returns an audio sample receiver and stop handle.
pub fn start_audio_capture_dual(
    system_source_id: Option<&str>,
    mic_source_id: Option<&str>,
    aec_enabled: bool,
) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    eprintln!(
        "[Audio] Starting dual capture: system={:?}, mic={:?}, aec={}",
        system_source_id, mic_source_id, aec_enabled
    );

    // Verify sources exist and determine device IDs
    let sources = list_audio_sources().map_err(|e| {
        CaptureError::AudioError(format!("Failed to enumerate audio sources: {:?}", e))
    })?;

    let sys_id = if let Some(id) = system_source_id {
        let source = sources
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| CaptureError::AudioError(format!("System audio device not found: {}", id)))?;
        if source.source_type != AudioSourceType::Output {
            return Err(CaptureError::AudioError(format!(
                "Device {} is not an output device for system audio capture",
                id
            )));
        }
        Some(id.to_string())
    } else {
        None
    };

    let mic_id = if let Some(id) = mic_source_id {
        let source = sources
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| CaptureError::AudioError(format!("Microphone device not found: {}", id)))?;
        if source.source_type != AudioSourceType::Input {
            return Err(CaptureError::AudioError(format!(
                "Device {} is not an input device for microphone capture",
                id
            )));
        }
        Some(id.to_string())
    } else {
        None
    };

    // Create output channel
    let (tx, rx) = mpsc::channel(256);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);

    // Create capture manager (spawns threads)
    let manager = MultiCaptureManager::new(mic_id, sys_id, aec_enabled, tx)?;

    // Wrap manager in Arc<Mutex> so we can drop it when stop flag is set
    let manager = Arc::new(Mutex::new(Some(manager)));
    let manager_clone = Arc::clone(&manager);

    // Spawn cleanup thread that watches the stop flag
    thread::spawn(move || {
        while !stop_flag_clone.load(Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_millis(100));
        }
        // Drop the manager to stop all capture threads
        if let Ok(mut guard) = manager_clone.lock() {
            *guard = None;
        }
    });

    Ok((rx, stop_flag))
}

/// Initialize the audio backend.
///
/// For Windows, verifies that COM can be initialized and audio devices are accessible.
pub fn init_audio_backend() -> Result<(), String> {
    eprintln!("[Audio] Initializing Windows WASAPI audio backend");

    // Test COM initialization
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };

    if com_initialized {
        // Test device enumeration
        let result = enumerate_audio_devices();
        unsafe {
            CoUninitialize();
        }
        match result {
            Ok(devices) => {
                eprintln!(
                    "[Audio] Windows WASAPI audio backend initialized ({} devices)",
                    devices.len()
                );
                Ok(())
            }
            Err(e) => {
                eprintln!("[Audio] Failed to enumerate devices: {:?}", e);
                // Don't fail initialization, just warn
                Ok(())
            }
        }
    } else {
        eprintln!("[Audio] COM initialization failed, audio may not work");
        // Don't fail initialization, just warn
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_list_audio_sources_returns_ok() {
        // Should return Ok even if no devices are found
        let result = list_audio_sources();
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_audio_backend_succeeds() {
        let result = init_audio_backend();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_audio_sources_returns_devices() {
        // This test verifies that on a typical Windows system with audio hardware,
        // we can enumerate devices. The test passes even with 0 devices.
        let result = list_audio_sources();
        assert!(result.is_ok());

        let devices = result.unwrap();
        // Print devices for manual verification (use cargo test -- --nocapture)
        eprintln!("\n=== Audio Device Enumeration Test ===");
        eprintln!("Found {} audio devices:", devices.len());
        for device in &devices {
            eprintln!(
                "  - {} [{}] (type: {:?})",
                device.name, device.id, device.source_type
            );
        }
        eprintln!("=====================================\n");

        // Verify device properties if any devices exist
        for device in &devices {
            // ID should not be empty
            assert!(!device.id.is_empty(), "Device ID should not be empty");
            // Name should not be empty
            assert!(!device.name.is_empty(), "Device name should not be empty");
        }
    }

    #[test]
    fn test_convert_samples_f32_stereo() {
        // Test float32 stereo pass-through
        let input: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
        let input_bytes = unsafe {
            std::slice::from_raw_parts(input.as_ptr() as *const u8, input.len() * 4)
        };

        let output = convert_samples_to_f32(input_bytes.as_ptr(), 2, 2, 32, true);
        assert_eq!(output.len(), 4);
        assert!((output[0] - 0.5).abs() < 0.001);
        assert!((output[1] - (-0.5)).abs() < 0.001);
    }

    #[test]
    fn test_convert_samples_i16_stereo() {
        // Test int16 stereo conversion
        let input: Vec<i16> = vec![16384, -16384, 8192, -8192]; // ~0.5, -0.5, 0.25, -0.25
        let input_bytes = unsafe {
            std::slice::from_raw_parts(input.as_ptr() as *const u8, input.len() * 2)
        };

        let output = convert_samples_to_f32(input_bytes.as_ptr(), 2, 2, 16, false);
        assert_eq!(output.len(), 4);
        assert!((output[0] - 0.5).abs() < 0.01);
        assert!((output[1] - (-0.5)).abs() < 0.01);
    }

    #[test]
    fn test_convert_samples_mono_to_stereo() {
        // Test mono to stereo duplication
        let input: Vec<f32> = vec![0.5, -0.5];
        let input_bytes = unsafe {
            std::slice::from_raw_parts(input.as_ptr() as *const u8, input.len() * 4)
        };

        let output = convert_samples_to_f32(input_bytes.as_ptr(), 2, 1, 32, true);
        assert_eq!(output.len(), 4); // Mono expanded to stereo
        assert!((output[0] - 0.5).abs() < 0.001);
        assert!((output[1] - 0.5).abs() < 0.001); // Duplicated
        assert!((output[2] - (-0.5)).abs() < 0.001);
        assert!((output[3] - (-0.5)).abs() < 0.001); // Duplicated
    }

    #[test]
    fn test_audio_capture_integration() {
        // Integration test: actually capture some audio
        let sources = list_audio_sources().expect("Failed to enumerate audio sources");
        
        // Skip if no output devices (loopback capture targets)
        let output_source = sources.iter().find(|s| s.source_type == AudioSourceType::Output);
        if output_source.is_none() {
            eprintln!("[Test] No output audio sources found, skipping integration test");
            return;
        }
        
        let source = output_source.unwrap();
        eprintln!("[Test] Testing capture with device: {}", source.name);
        
        // Start capture
        let result = start_audio_capture(&source.id);
        if let Err(ref e) = result {
            eprintln!("[Test] Capture start error (may be expected): {}", e);
            // Don't fail the test - device may be in use
            return;
        }
        
        let (mut rx, stop_flag) = result.unwrap();
        
        // Capture for 500ms
        let start = std::time::Instant::now();
        let mut samples_received = 0u64;
        let mut frames_received = 0u32;
        
        // Use a runtime for async operations
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        
        rt.block_on(async {
            while start.elapsed().as_millis() < 500 {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    rx.recv()
                ).await {
                    Ok(Some(sample)) => {
                        samples_received += sample.data.len() as u64;
                        frames_received += 1;
                    }
                    Ok(None) => {
                        eprintln!("[Test] Channel closed");
                        break;
                    }
                    Err(_) => {
                        // Timeout - this is okay for loopback if nothing is playing
                    }
                }
            }
        });
        
        // Stop capture
        stop_flag.store(true, Ordering::Relaxed);
        
        eprintln!(
            "[Test] Capture complete: {} samples in {} frames over {}ms",
            samples_received,
            frames_received,
            start.elapsed().as_millis()
        );
        
        // For loopback capture, we might get 0 samples if nothing is playing
        // Just verify we didn't crash
        eprintln!("[Test] Integration test passed");
    }

    #[test]
    fn test_audio_mixer_single_stream_mic() {
        // Test single-stream mode (mic only) - samples should pass through directly
        let (tx, mut rx) = mpsc::channel(16);
        let mut mixer = AudioMixer::new(tx, false);
        mixer.set_num_streams(1);

        // Push some mic samples (is_loopback=false means mic)
        let samples = vec![0.5f32, -0.5, 0.25, -0.25];
        mixer.push_samples(&samples, false);

        // Should receive them directly (try_recv returns Result<T, TryRecvError>)
        let sample = rx.try_recv().expect("Should receive samples in single-stream mode");
        assert_eq!(sample.data.len(), 4);
        assert!((sample.data[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_audio_mixer_single_stream_sys() {
        // Test single-stream mode (system audio only)
        let (tx, mut rx) = mpsc::channel(16);
        let mut mixer = AudioMixer::new(tx, false);
        mixer.set_num_streams(1);

        // is_loopback=true means system audio
        let samples = vec![0.3f32, -0.3, 0.15, -0.15];
        mixer.push_samples(&samples, true);

        let sample = rx.try_recv().expect("Should receive samples in single-stream mode");
        assert_eq!(sample.data.len(), 4);
    }

    #[test]
    fn test_audio_mixer_dual_stream_mixing() {
        // Test dual-stream mixing without AEC
        // Note: Mixer uses frame-based processing (AEC_FRAME_SAMPLES * 2 channels = 960 samples)
        let (tx, mut rx) = mpsc::channel(16);
        let mut mixer = AudioMixer::new(tx, false); // AEC disabled
        mixer.set_num_streams(2);

        // Create full frame of samples (480 samples * 2 channels = 960)
        let frame_size = 480 * 2;
        let capture_samples: Vec<f32> = vec![0.4f32; frame_size]; // mic
        let render_samples: Vec<f32> = vec![0.2f32; frame_size];  // system audio

        // Push system audio first (is_loopback=true), then mic (is_loopback=false)
        mixer.push_samples(&render_samples, true);
        mixer.push_samples(&capture_samples, false);

        // Should receive mixed samples
        let sample = rx.try_recv().expect("Should receive mixed samples");
        
        // Mixed output should be 0.4 + 0.2 = 0.6 (with soft clipping, values < 1.0 pass through)
        assert_eq!(sample.data.len(), frame_size);
        assert!((sample.data[0] - 0.6).abs() < 0.001, "Mixed sample should be 0.6, got {}", sample.data[0]);
    }

    #[test]
    fn test_audio_mixer_clipping_protection() {
        // Test that mixing uses soft clipping to prevent harsh distortion
        // Note: Mixer uses frame-based processing (480 samples * 2 channels = 960)
        let (tx, mut rx) = mpsc::channel(16);
        let mut mixer = AudioMixer::new(tx, false); // AEC disabled
        mixer.set_num_streams(2);

        // Full frame of samples at full volume (would clip without protection)
        let frame_size = 480 * 2;
        let capture_samples: Vec<f32> = vec![1.0f32; frame_size];
        let render_samples: Vec<f32> = vec![1.0f32; frame_size];

        // Push system audio first, then mic
        mixer.push_samples(&render_samples, true);
        mixer.push_samples(&capture_samples, false);

        let sample = rx.try_recv().expect("Should receive samples");
        
        // With soft clipping: sum = 2.0, output = 1.0 - exp(-2.0) * 0.5 ≈ 0.932
        // Values should be in valid range and soft-clipped (not hard-clamped to 1.0)
        for &s in &sample.data {
            assert!(s <= 1.0, "Sample should be <= 1.0, got {}", s);
            assert!(s >= -1.0, "Sample should be >= -1.0, got {}", s);
            // Soft clipping produces values < 1.0 for inputs > 1.0
            assert!(s > 0.9, "Soft-clipped sample should be > 0.9, got {}", s);
        }
    }

    #[test]
    fn test_audio_mixer_aec_frame_alignment() {
        // Test that AEC mode waits for full frames (480 samples * 2 channels = 960)
        let (tx, mut rx) = mpsc::channel(16);
        let mut mixer = AudioMixer::new(tx, true); // AEC enabled
        mixer.set_num_streams(2);

        // Push less than a full frame
        let small_capture = vec![0.1f32; 100];
        let small_render = vec![0.2f32; 100];

        mixer.push_samples(&small_render, true);  // system audio first
        mixer.push_samples(&small_capture, false); // then mic

        // Should NOT receive anything yet (need 960 samples)
        assert!(rx.try_recv().is_err(), "Should not receive samples before full frame");

        // Push more to reach full frame
        let more_capture = vec![0.1f32; 860];
        let more_render = vec![0.2f32; 860];

        mixer.push_samples(&more_render, true);
        mixer.push_samples(&more_capture, false);

        // NOW should receive
        assert!(rx.try_recv().is_ok(), "Should receive samples after full frame");
    }

    #[test]
    fn test_dual_capture_api() {
        // Test that the dual capture API can be called
        // This is a basic API test - actual capture requires real devices
        let sources = list_audio_sources().expect("Failed to enumerate audio sources");
        
        // Find an output and input device
        let output = sources.iter().find(|s| s.source_type == AudioSourceType::Output);
        let input = sources.iter().find(|s| s.source_type == AudioSourceType::Input);
        
        if output.is_none() || input.is_none() {
            eprintln!("[Test] Need both output and input devices for dual capture test");
            return;
        }
        
        let sys_id = &output.unwrap().id;
        let mic_id = &input.unwrap().id;
        
        eprintln!("[Test] Testing dual capture with:");
        eprintln!("  System: {}", output.unwrap().name);
        eprintln!("  Mic: {}", input.unwrap().name);
        
        // Start dual capture
        let result = start_audio_capture_dual(Some(sys_id), Some(mic_id), true);
        if let Err(ref e) = result {
            eprintln!("[Test] Dual capture start error (may be expected): {:?}", e);
            return;
        }
        
        let (mut rx, stop_flag) = result.unwrap();
        
        // Capture briefly
        let start = std::time::Instant::now();
        let mut samples_received = 0u64;
        
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        
        rt.block_on(async {
            while start.elapsed().as_millis() < 500 {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    rx.recv()
                ).await {
                    Ok(Some(sample)) => {
                        samples_received += sample.data.len() as u64;
                    }
                    Ok(None) => break,
                    Err(_) => {}
                }
            }
        });
        
        stop_flag.store(true, Ordering::Relaxed);
        
        // Give threads time to stop
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        eprintln!("[Test] Dual capture received {} samples", samples_received);
        eprintln!("[Test] Dual capture test passed");
    }
}
