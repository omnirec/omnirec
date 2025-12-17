//! PipeWire-based audio capture for Linux.
//!
//! This module provides audio capture from input devices (microphones) and
//! system audio (sink monitors) using PipeWire. Audio is captured as 48kHz
//! stereo f32 samples for muxing with video in the recording pipeline.

use pipewire::{
    context::Context,
    main_loop::MainLoop,
    properties::properties,
    spa::{
        param::audio::{AudioFormat, AudioInfoRaw},
        pod::Pod,
        utils::Direction,
    },
    stream::{Stream, StreamFlags},
    types::ObjectType,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{AudioReceiver, AudioSample, AudioSource, AudioSourceType, StopHandle};

/// Commands sent to the PipeWire audio thread.
#[derive(Debug)]
enum AudioCommand {
    /// Start capturing from the specified source
    StartCapture { source_id: u32 },
    /// Stop capture
    StopCapture,
}

/// Handle to the PipeWire audio backend.
///
/// This manages the PipeWire thread and provides device enumeration.
pub struct PipeWireAudioBackend {
    /// Channel to send commands to PipeWire thread
    cmd_tx: std_mpsc::Sender<AudioCommand>,
    /// Cached input devices (microphones)
    input_devices: Arc<Mutex<Vec<AudioSource>>>,
    /// Cached output devices (sink monitors for system audio)
    output_devices: Arc<Mutex<Vec<AudioSource>>>,
    /// Thread handle
    _thread_handle: JoinHandle<()>,
    /// Current sample rate
    sample_rate: Arc<Mutex<u32>>,
    /// Async sender for audio samples (created per capture session)
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>>,
}

impl PipeWireAudioBackend {
    /// Create and start the PipeWire audio backend.
    pub fn new() -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = std_mpsc::channel();
        let input_devices = Arc::new(Mutex::new(Vec::new()));
        let output_devices = Arc::new(Mutex::new(Vec::new()));
        let sample_rate = Arc::new(Mutex::new(48000u32));
        let audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>> = Arc::new(Mutex::new(None));

        let input_devices_clone = Arc::clone(&input_devices);
        let output_devices_clone = Arc::clone(&output_devices);
        let sample_rate_clone = Arc::clone(&sample_rate);
        let audio_tx_clone = Arc::clone(&audio_tx);

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_pipewire_audio_thread(
                cmd_rx,
                input_devices_clone,
                output_devices_clone,
                sample_rate_clone,
                audio_tx_clone,
            ) {
                eprintln!("[Audio] PipeWire thread error: {}", e);
            }
        });

        // Give PipeWire a moment to enumerate devices
        thread::sleep(std::time::Duration::from_millis(200));

        Ok(Self {
            cmd_tx,
            input_devices,
            output_devices,
            _thread_handle: thread_handle,
            sample_rate,
            audio_tx,
        })
    }

    /// List all available audio sources (inputs and output monitors).
    pub fn list_audio_sources(&self) -> Vec<AudioSource> {
        let mut sources = Vec::new();

        // Add input devices (microphones)
        if let Ok(inputs) = self.input_devices.lock() {
            sources.extend(inputs.clone());
        }

        // Add output devices (system audio monitors)
        if let Ok(outputs) = self.output_devices.lock() {
            sources.extend(outputs.clone());
        }

        sources
    }

    /// Get current sample rate.
    pub fn sample_rate(&self) -> u32 {
        *self.sample_rate.lock().unwrap()
    }

    /// Start audio capture from the specified source.
    ///
    /// Returns an audio sample receiver and stop handle.
    pub fn start_capture(&self, source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        // Parse source ID as u32 (PipeWire node ID)
        let node_id: u32 = source_id.parse().map_err(|_| {
            CaptureError::AudioError(format!("Invalid audio source ID: {}", source_id))
        })?;

        // Create channel for audio samples
        let (tx, rx) = mpsc::channel(256);
        
        // Store the sender for the PipeWire thread to use
        {
            let mut audio_tx = self.audio_tx.lock().unwrap();
            *audio_tx = Some(tx);
        }

        // Create stop handle
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Send start command to PipeWire thread
        self.cmd_tx
            .send(AudioCommand::StartCapture { source_id: node_id })
            .map_err(|e| CaptureError::AudioError(format!("Failed to start audio capture: {}", e)))?;

        // Clone sender for cleanup
        let cmd_tx = self.cmd_tx.clone();
        let audio_tx_for_cleanup = Arc::clone(&self.audio_tx);
        let stop_flag_clone = Arc::clone(&stop_flag);

        // Spawn cleanup task that watches the stop flag
        thread::spawn(move || {
            while !stop_flag_clone.load(Ordering::Relaxed) {
                thread::sleep(std::time::Duration::from_millis(100));
            }
            // Stop capture when flag is set
            let _ = cmd_tx.send(AudioCommand::StopCapture);
            // Clear the audio sender
            if let Ok(mut tx) = audio_tx_for_cleanup.lock() {
                *tx = None;
            }
        });

        Ok((rx, stop_flag))
    }
}

/// Run the PipeWire main loop thread for audio.
fn run_pipewire_audio_thread(
    cmd_rx: std_mpsc::Receiver<AudioCommand>,
    input_devices: Arc<Mutex<Vec<AudioSource>>>,
    output_devices: Arc<Mutex<Vec<AudioSource>>>,
    sample_rate: Arc<Mutex<u32>>,
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>>,
) -> Result<(), String> {
    // Initialize PipeWire
    pipewire::init();

    let mainloop = MainLoop::new(None).map_err(|e| format!("Failed to create main loop: {}", e))?;
    let context = Context::new(&mainloop).map_err(|e| format!("Failed to create context: {}", e))?;
    let core = context
        .connect(None)
        .map_err(|e| format!("Failed to connect to PipeWire: {}", e))?;
    let registry = core
        .get_registry()
        .map_err(|e| format!("Failed to get registry: {}", e))?;

    // Device maps for enumeration
    let input_map: Rc<RefCell<HashMap<u32, AudioSource>>> = Rc::new(RefCell::new(HashMap::new()));
    let output_map: Rc<RefCell<HashMap<u32, AudioSource>>> = Rc::new(RefCell::new(HashMap::new()));

    // Setup registry listener for device discovery
    let input_map_clone = Rc::clone(&input_map);
    let output_map_clone = Rc::clone(&output_map);
    let input_devices_clone = Arc::clone(&input_devices);
    let output_devices_clone = Arc::clone(&output_devices);

    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            if global.type_ == ObjectType::Node {
                if let Some(props) = &global.props {
                    let media_class = props.get("media.class").unwrap_or("");
                    let node_name = props.get("node.name").unwrap_or("Unknown");
                    let node_desc = props.get("node.description").unwrap_or(node_name);

                    if media_class == "Audio/Source" {
                        // Input device (microphone)
                        let source = AudioSource {
                            id: global.id.to_string(),
                            name: node_desc.to_string(),
                            source_type: AudioSourceType::Input,
                        };
                        eprintln!("[Audio] Found input device: {} (ID: {})", source.name, source.id);
                        input_map_clone.borrow_mut().insert(global.id, source);
                        // Update shared list
                        let devices: Vec<_> = input_map_clone.borrow().values().cloned().collect();
                        *input_devices_clone.lock().unwrap() = devices;
                    } else if media_class == "Audio/Sink" {
                        // Output device - we can capture its monitor for system audio
                        let source = AudioSource {
                            id: global.id.to_string(),
                            name: format!("{} (Monitor)", node_desc),
                            source_type: AudioSourceType::Output,
                        };
                        eprintln!("[Audio] Found output device: {} (ID: {})", source.name, source.id);
                        output_map_clone.borrow_mut().insert(global.id, source);
                        // Update shared list
                        let devices: Vec<_> = output_map_clone.borrow().values().cloned().collect();
                        *output_devices_clone.lock().unwrap() = devices;
                    }
                }
            }
        })
        .global_remove({
            let input_map = Rc::clone(&input_map);
            let output_map = Rc::clone(&output_map);
            let input_devices = Arc::clone(&input_devices);
            let output_devices = Arc::clone(&output_devices);
            move |id| {
                if input_map.borrow_mut().remove(&id).is_some() {
                    eprintln!("[Audio] Input device removed: {}", id);
                    let devices: Vec<_> = input_map.borrow().values().cloned().collect();
                    *input_devices.lock().unwrap() = devices;
                }
                if output_map.borrow_mut().remove(&id).is_some() {
                    eprintln!("[Audio] Output device removed: {}", id);
                    let devices: Vec<_> = output_map.borrow().values().cloned().collect();
                    *output_devices.lock().unwrap() = devices;
                }
            }
        })
        .register();

    // Track sink IDs to know which devices need STREAM_CAPTURE_SINK
    let sink_ids: Rc<RefCell<std::collections::HashSet<u32>>> = Rc::new(RefCell::new(std::collections::HashSet::new()));
    
    // Keep sink_ids in sync with output_map
    let sink_ids_for_sync = Rc::clone(&sink_ids);
    let output_map_for_sync = Rc::clone(&output_map);

    // Active capture stream state
    struct CaptureState {
        _stream: Option<Stream>,
        sample_rate: Arc<Mutex<u32>>,
        audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>>,
    }
    
    let capture_state = Rc::new(RefCell::new(CaptureState {
        _stream: None,
        sample_rate: Arc::clone(&sample_rate),
        audio_tx: Arc::clone(&audio_tx),
    }));

    // Setup command receiver using a timer that polls the channel
    let core_ref = Rc::new(core);
    let core_for_timer = Rc::clone(&core_ref);
    let capture_state_for_timer = Rc::clone(&capture_state);

    // Create a timer source to poll for commands
    let timer_source = mainloop.loop_().add_timer({
        move |_elapsed| {
            // Update sink_ids from output_map
            {
                let mut sink_ids = sink_ids_for_sync.borrow_mut();
                sink_ids.clear();
                for id in output_map_for_sync.borrow().keys() {
                    sink_ids.insert(*id);
                }
            }

            // Poll for commands
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    AudioCommand::StartCapture { source_id } => {
                        eprintln!("[Audio] Starting capture for source ID: {}", source_id);
                        
                        // Check if this is a sink (system audio)
                        let is_sink = sink_ids.borrow().contains(&source_id);
                        
                        let mut state = capture_state_for_timer.borrow_mut();
                        
                        // Stop any existing stream
                        state._stream = None;
                        
                        // Create new capture stream
                        match create_audio_capture_stream(
                            &core_for_timer,
                            source_id,
                            is_sink,
                            Arc::clone(&state.sample_rate),
                            Arc::clone(&state.audio_tx),
                        ) {
                            Ok(stream) => {
                                eprintln!("[Audio] Capture stream created successfully");
                                state._stream = Some(stream);
                            }
                            Err(e) => {
                                eprintln!("[Audio] Failed to create capture stream: {}", e);
                            }
                        }
                    }
                    AudioCommand::StopCapture => {
                        eprintln!("[Audio] Stopping capture");
                        capture_state_for_timer.borrow_mut()._stream = None;
                    }
                }
            }
        }
    });

    // Set timer to fire every 10ms
    timer_source.update_timer(
        Some(std::time::Duration::from_millis(10)),
        Some(std::time::Duration::from_millis(10)),
    );

    // Run the main loop (blocks until quit)
    mainloop.run();

    Ok(())
}

/// Create an audio format pod for stream connection.
fn create_audio_format_pod() -> Vec<u8> {
    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    // Leave rate and channels unset to accept native graph format
    // PipeWire will typically provide 48kHz stereo

    let obj = pipewire::spa::pod::Object {
        type_: pipewire::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: pipewire::spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };

    pipewire::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pipewire::spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner()
}

/// Create a capture stream that sends samples to the audio channel.
fn create_audio_capture_stream(
    core: &pipewire::core::Core,
    device_id: u32,
    capture_sink: bool,
    sample_rate: Arc<Mutex<u32>>,
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>>,
) -> Result<Stream, String> {
    let stream_name = if capture_sink {
        "omnirec-system-audio"
    } else {
        "omnirec-audio-input"
    };

    let props = if capture_sink {
        // For system audio capture, use STREAM_CAPTURE_SINK
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Music",
            *pipewire::keys::STREAM_CAPTURE_SINK => "true",
        }
    } else {
        // For input devices (microphones)
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Music",
        }
    };

    let stream = Stream::new(core, stream_name, props)
        .map_err(|e| format!("Failed to create stream: {}", e))?;

    // Track format info from param_changed
    let format_info: Rc<RefCell<AudioInfoRaw>> = Rc::new(RefCell::new(AudioInfoRaw::default()));
    let format_info_for_param = Rc::clone(&format_info);
    let sample_rate_for_param = Arc::clone(&sample_rate);
    
    // Current channels (updated from param_changed)
    let channels: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));
    let channels_for_param = Rc::clone(&channels);
    let channels_for_process = Rc::clone(&channels);
    
    let audio_tx_for_process = Arc::clone(&audio_tx);
    let sample_rate_for_process = Arc::clone(&sample_rate);

    let listener = stream
        .add_local_listener_with_user_data(())
        .param_changed(move |_stream, _user_data, id, param| {
            let Some(param) = param else { return };

            if id != pipewire::spa::param::ParamType::Format.as_raw() {
                return;
            }

            // Parse the format
            if let Ok((media_type, media_subtype)) =
                pipewire::spa::param::format_utils::parse_format(param)
            {
                use pipewire::spa::param::format::{MediaSubtype, MediaType};
                if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                    return;
                }

                if format_info_for_param.borrow_mut().parse(param).is_ok() {
                    let rate = format_info_for_param.borrow().rate();
                    let ch = format_info_for_param.borrow().channels();
                    eprintln!("[Audio] Stream format: rate={}, channels={}", rate, ch);
                    *sample_rate_for_param.lock().unwrap() = rate;
                    *channels_for_param.borrow_mut() = ch;
                }
            }
        })
        .state_changed(move |_stream, _user_data, old, new| {
            eprintln!("[Audio] Stream state: {:?} -> {:?}", old, new);
        })
        .process(move |stream, _user_data| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    return;
                }

                let data = &mut datas[0];
                // Get chunk info first
                let chunk_size = data.chunk().size() as usize;
                let n_samples = chunk_size / mem::size_of::<f32>();

                if n_samples == 0 {
                    return;
                }

                if let Some(samples_data) = data.data() {
                    // Convert bytes to f32 samples
                    let samples: Vec<f32> = samples_data[..chunk_size]
                        .chunks_exact(4)
                        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                        .collect();

                    if !samples.is_empty() {
                        // Get current format info
                        let rate = *sample_rate_for_process.lock().unwrap();
                        let ch = *channels_for_process.borrow();
                        
                        // Send audio sample
                        if let Ok(guard) = audio_tx_for_process.lock() {
                            if let Some(tx) = guard.as_ref() {
                                let sample = AudioSample {
                                    data: samples,
                                    sample_rate: rate,
                                    channels: ch,
                                };
                                // Use try_send to avoid blocking
                                let _ = tx.try_send(sample);
                            }
                        }
                    }
                }
            }
        })
        .register()
        .map_err(|e| format!("Failed to register stream listener: {}", e))?;

    // Create audio format parameters
    let format_pod = create_audio_format_pod();
    let mut params = [Pod::from_bytes(&format_pod).unwrap()];

    // Connect to device
    let flags = StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS;

    stream
        .connect(Direction::Input, Some(device_id), flags, &mut params)
        .map_err(|e| format!("Failed to connect stream: {}", e))?;

    // Leak the listener to keep it alive - it will be cleaned up when stream is dropped
    std::mem::forget(listener);

    Ok(stream)
}

// Global audio backend instance (initialized once)
static AUDIO_BACKEND: once_cell::sync::OnceCell<PipeWireAudioBackend> = once_cell::sync::OnceCell::new();

/// Initialize the global audio backend (call once at app startup).
pub fn init_audio_backend() -> Result<(), String> {
    if AUDIO_BACKEND.get().is_some() {
        eprintln!("[Audio] Backend already initialized");
        return Ok(());
    }

    eprintln!("[Audio] Initializing PipeWire audio backend...");
    let backend = PipeWireAudioBackend::new()?;
    AUDIO_BACKEND
        .set(backend)
        .map_err(|_| "Audio backend already set")?;
    eprintln!("[Audio] PipeWire audio backend initialized");
    Ok(())
}

/// Get the global audio backend.
pub fn get_audio_backend() -> Option<&'static PipeWireAudioBackend> {
    AUDIO_BACKEND.get()
}

/// List all available audio sources.
pub fn list_audio_sources() -> Result<Vec<AudioSource>, EnumerationError> {
    let backend = get_audio_backend().ok_or_else(|| {
        EnumerationError::PlatformError("Audio backend not initialized".to_string())
    })?;
    Ok(backend.list_audio_sources())
}

/// Start audio capture from the specified source.
pub fn start_audio_capture(source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    let backend = get_audio_backend().ok_or_else(|| {
        CaptureError::AudioError("Audio backend not initialized".to_string())
    })?;
    backend.start_capture(source_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_source_type_serialization() {
        let input = AudioSourceType::Input;
        let output = AudioSourceType::Output;
        
        assert_eq!(serde_json::to_string(&input).unwrap(), "\"input\"");
        assert_eq!(serde_json::to_string(&output).unwrap(), "\"output\"");
    }
}
