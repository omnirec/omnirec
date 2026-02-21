//! PipeWire-based audio capture for Linux with dual-source support and AEC.
//!
//! This module provides audio capture from input devices (microphones) and
//! system audio (sink monitors) using PipeWire. When both sources are active,
//! they are mixed together with optional Acoustic Echo Cancellation (AEC).
//! Audio is captured as 48kHz stereo f32 samples for muxing with video.

use aec3::voip::VoipAec3;
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
use crate::capture::types::{AudioReceiver, AudioSample, StopHandle};
use crate::capture::{AudioSource, AudioSourceType};

/// AEC3 frame size: 10ms at 48kHz = 480 samples per channel
const AEC_FRAME_SAMPLES: usize = 480;

/// Commands sent to the PipeWire audio thread.
#[derive(Debug)]
enum AudioCommand {
    /// Start capturing from up to two sources (system audio and/or microphone)
    StartCaptureSources {
        system_source_id: Option<u32>,
        mic_source_id: Option<u32>,
    },
    /// Stop all capture
    StopCapture,
}

/// Audio samples output from the mixer
struct MixedAudioSamples {
    samples: Vec<f32>,
    channels: u16,
}

/// Mixer state for combining audio from multiple streams with optional AEC
struct AudioMixer {
    /// Buffer for microphone samples (primary)
    buffer_mic: Vec<f32>,
    /// Buffer for system audio samples (reference)
    buffer_sys: Vec<f32>,
    /// Number of active streams (1 or 2)
    num_streams: usize,
    /// Channels per stream
    channels: u16,
    /// Output sender
    output_tx: std_mpsc::Sender<MixedAudioSamples>,
    /// Flag to enable/disable AEC (shared with main thread)
    aec_enabled: Arc<Mutex<bool>>,
    /// AEC3 instance (created when 2 streams active)
    aec: Option<VoipAec3>,
}

impl AudioMixer {
    fn new(output_tx: std_mpsc::Sender<MixedAudioSamples>, aec_enabled: Arc<Mutex<bool>>) -> Self {
        Self {
            buffer_mic: Vec::new(),
            buffer_sys: Vec::new(),
            num_streams: 0,
            channels: 2,
            output_tx,
            aec_enabled,
            aec: None,
        }
    }

    fn set_num_streams(&mut self, num: usize) {
        self.num_streams = num;
        self.buffer_mic.clear();
        self.buffer_sys.clear();

        // Create AEC3 pipeline when we have 2 streams
        if num == 2 {
            match VoipAec3::builder(48000, self.channels as usize, self.channels as usize)
                .enable_high_pass(true)
                .build()
            {
                Ok(aec) => {
                    eprintln!(
                        "[AudioMixer] AEC3 initialized: 48kHz, {} channels, {}ms frames",
                        self.channels,
                        AEC_FRAME_SAMPLES * 1000 / 48000
                    );
                    self.aec = Some(aec);
                }
                Err(e) => {
                    eprintln!("[AudioMixer] Failed to initialize AEC3: {:?}", e);
                    self.aec = None;
                }
            }
        } else {
            self.aec = None;
        }
    }

    fn set_channels(&mut self, channels: u16) {
        self.channels = channels;
    }

    /// Add samples from microphone stream
    fn push_mic(&mut self, samples: &[f32]) {
        if self.num_streams == 1 {
            // Only one stream (mic only) - send directly
            let _ = self.output_tx.send(MixedAudioSamples {
                samples: samples.to_vec(),
                channels: self.channels,
            });
        } else {
            // Two streams - buffer and mix
            self.buffer_mic.extend_from_slice(samples);
            self.try_mix_and_send();
        }
    }

    /// Add samples from system audio stream
    fn push_sys(&mut self, samples: &[f32]) {
        if self.num_streams == 1 {
            // Only one stream (system audio only) - send directly
            let _ = self.output_tx.send(MixedAudioSamples {
                samples: samples.to_vec(),
                channels: self.channels,
            });
        } else {
            // Two streams - buffer and mix
            self.buffer_sys.extend_from_slice(samples);
            self.try_mix_and_send();
        }
    }

    /// Try to mix available samples and send
    fn try_mix_and_send(&mut self) {
        let aec_enabled = *self.aec_enabled.lock().unwrap();

        // AEC3 requires exactly frame_samples * channels samples per frame
        let frame_size = AEC_FRAME_SAMPLES * self.channels as usize;

        // Minimum samples needed: one full frame for AEC, or any aligned amount without
        let min_samples = if aec_enabled && self.aec.is_some() {
            frame_size
        } else {
            self.channels as usize // At least one sample per channel
        };

        // Process frames while we have enough data from both streams
        while self.buffer_mic.len() >= min_samples && self.buffer_sys.len() >= min_samples {
            let process_count = if aec_enabled && self.aec.is_some() {
                frame_size
            } else {
                // Without AEC, process all available (aligned to channels)
                let available = std::cmp::min(self.buffer_mic.len(), self.buffer_sys.len());
                (available / self.channels as usize) * self.channels as usize
            };

            if process_count == 0 {
                break;
            }

            // Extract interleaved samples to process
            let mic_samples: Vec<f32> = self.buffer_mic.drain(0..process_count).collect();
            let sys_samples: Vec<f32> = self.buffer_sys.drain(0..process_count).collect();

            // Apply AEC if enabled
            let processed_mic = if aec_enabled {
                if let Some(ref mut aec) = self.aec {
                    let mut out = vec![0.0f32; mic_samples.len()];

                    // Process capture (mic) with render (system audio) as reference
                    // The render frame is what's being played through speakers
                    // The capture frame is what the mic picks up (including echo)
                    match aec.process(&mic_samples, Some(&sys_samples), false, &mut out) {
                        Ok(_metrics) => out,
                        Err(e) => {
                            eprintln!("[AudioMixer] AEC3 process error: {:?}", e);
                            mic_samples
                        }
                    }
                } else {
                    mic_samples
                }
            } else {
                mic_samples
            };

            // Mix processed mic with system audio (0.5 gain each to prevent clipping)
            let output: Vec<f32> = processed_mic
                .iter()
                .zip(sys_samples.iter())
                .map(|(&mic, &sys)| ((mic + sys) * 0.5).clamp(-1.0, 1.0))
                .collect();

            // Send mixed output
            let _ = self.output_tx.send(MixedAudioSamples {
                samples: output,
                channels: self.channels,
            });
        }
    }
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
    #[allow(dead_code)]
    sample_rate: Arc<Mutex<u32>>,
    /// Async sender for audio samples (created per capture session)
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>>,
    /// AEC enabled flag (shared with mixer)
    aec_enabled: Arc<Mutex<bool>>,
}

impl PipeWireAudioBackend {
    /// Create and start the PipeWire audio backend.
    pub fn new() -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = std_mpsc::channel();
        let input_devices = Arc::new(Mutex::new(Vec::new()));
        let output_devices = Arc::new(Mutex::new(Vec::new()));
        let sample_rate = Arc::new(Mutex::new(48000u32));
        let audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>> = Arc::new(Mutex::new(None));
        let aec_enabled = Arc::new(Mutex::new(true)); // AEC enabled by default

        let input_devices_clone = Arc::clone(&input_devices);
        let output_devices_clone = Arc::clone(&output_devices);
        let sample_rate_clone = Arc::clone(&sample_rate);
        let audio_tx_clone = Arc::clone(&audio_tx);
        let aec_enabled_clone = Arc::clone(&aec_enabled);

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_pipewire_audio_thread(
                cmd_rx,
                input_devices_clone,
                output_devices_clone,
                sample_rate_clone,
                audio_tx_clone,
                aec_enabled_clone,
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
            aec_enabled,
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
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        *self.sample_rate.lock().unwrap()
    }

    /// Set AEC enabled state.
    pub fn set_aec_enabled(&self, enabled: bool) {
        *self.aec_enabled.lock().unwrap() = enabled;
        eprintln!("[Audio] AEC enabled: {}", enabled);
    }

    /// Start audio capture from up to two sources.
    ///
    /// - `system_source_id`: System audio (output monitor) source ID, or None
    /// - `mic_source_id`: Microphone source ID, or None
    ///
    /// Returns an audio sample receiver and stop handle.
    pub fn start_capture_dual(
        &self,
        system_source_id: Option<&str>,
        mic_source_id: Option<&str>,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        // Parse source IDs as u32 (PipeWire node IDs)
        let sys_id: Option<u32> = system_source_id
            .map(|id| {
                id.parse().map_err(|_| {
                    CaptureError::AudioError(format!("Invalid system audio source ID: {}", id))
                })
            })
            .transpose()?;

        let mic_id: Option<u32> = mic_source_id
            .map(|id| {
                id.parse().map_err(|_| {
                    CaptureError::AudioError(format!("Invalid microphone source ID: {}", id))
                })
            })
            .transpose()?;

        // Must have at least one source
        if sys_id.is_none() && mic_id.is_none() {
            return Err(CaptureError::AudioError(
                "At least one audio source must be specified".to_string(),
            ));
        }

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
            .send(AudioCommand::StartCaptureSources {
                system_source_id: sys_id,
                mic_source_id: mic_id,
            })
            .map_err(|e| {
                CaptureError::AudioError(format!("Failed to start audio capture: {}", e))
            })?;

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

    /// Start audio capture from the specified source (backward compatible).
    ///
    /// Returns an audio sample receiver and stop handle.
    #[allow(dead_code)]
    pub fn start_capture(
        &self,
        source_id: &str,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        // Determine if this is a system audio source or microphone
        let is_output = {
            let outputs = self.output_devices.lock().unwrap();
            outputs.iter().any(|s| s.id == source_id)
        };

        if is_output {
            self.start_capture_dual(Some(source_id), None)
        } else {
            self.start_capture_dual(None, Some(source_id))
        }
    }
}

/// Held stream state - keeps stream and listener alive
struct ActiveStream {
    _stream: Stream,
}

/// Internal state for the PipeWire thread
struct PwThreadState {
    /// Active streams (kept alive)
    streams: Vec<ActiveStream>,
    /// Sample rate (updated from param_changed)
    sample_rate: Arc<Mutex<u32>>,
    /// Set of sink (system audio) device IDs
    sink_ids: Rc<RefCell<std::collections::HashSet<u32>>>,
}

/// Run the PipeWire main loop thread for audio.
fn run_pipewire_audio_thread(
    cmd_rx: std_mpsc::Receiver<AudioCommand>,
    input_devices: Arc<Mutex<Vec<AudioSource>>>,
    output_devices: Arc<Mutex<Vec<AudioSource>>>,
    sample_rate: Arc<Mutex<u32>>,
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioSample>>>>,
    aec_enabled: Arc<Mutex<bool>>,
) -> Result<(), String> {
    // Initialize PipeWire
    pipewire::init();

    let mainloop = MainLoop::new(None).map_err(|e| format!("Failed to create main loop: {}", e))?;
    let context =
        Context::new(&mainloop).map_err(|e| format!("Failed to create context: {}", e))?;
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
                        eprintln!(
                            "[Audio] Found input device: {} (ID: {})",
                            source.name, source.id
                        );
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
                        eprintln!(
                            "[Audio] Found output device: {} (ID: {})",
                            source.name, source.id
                        );
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
    let sink_ids: Rc<RefCell<std::collections::HashSet<u32>>> =
        Rc::new(RefCell::new(std::collections::HashSet::new()));

    // Keep sink_ids in sync with output_map
    let sink_ids_for_sync = Rc::clone(&sink_ids);
    let output_map_for_sync = Rc::clone(&output_map);

    // Create channel for mixer output
    let (mixer_tx, mixer_rx) = std_mpsc::channel::<MixedAudioSamples>();

    // Create mixer with AEC enabled flag
    let mixer = Rc::new(RefCell::new(AudioMixer::new(
        mixer_tx,
        Arc::clone(&aec_enabled),
    )));

    // Thread state
    let state = Rc::new(RefCell::new(PwThreadState {
        streams: Vec::new(),
        sample_rate: Arc::clone(&sample_rate),
        sink_ids: Rc::clone(&sink_ids),
    }));

    // Setup command receiver using a timer that polls the channel
    let core_ref = Rc::new(core);
    let core_for_timer = Rc::clone(&core_ref);
    let state_for_timer = Rc::clone(&state);
    let mixer_for_timer = Rc::clone(&mixer);
    let audio_tx_for_forward = Arc::clone(&audio_tx);

    // Create a timer source to poll for commands and forward mixer output
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

            // Forward mixer output to async channel
            while let Ok(mixed) = mixer_rx.try_recv() {
                if let Ok(guard) = audio_tx_for_forward.lock() {
                    if let Some(tx) = guard.as_ref() {
                        let sample = AudioSample {
                            data: mixed.samples,
                            sample_rate: 48000,
                            channels: mixed.channels as u32,
                        };
                        let _ = tx.try_send(sample);
                    }
                }
            }

            // Poll for commands
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    AudioCommand::StartCaptureSources {
                        system_source_id,
                        mic_source_id,
                    } => {
                        eprintln!(
                            "[Audio] Starting capture: system={:?}, mic={:?}",
                            system_source_id, mic_source_id
                        );

                        // Count how many streams we'll have
                        let num_streams =
                            system_source_id.is_some() as usize + mic_source_id.is_some() as usize;

                        let mut state = state_for_timer.borrow_mut();

                        // Clear existing streams
                        state.streams.clear();

                        // Configure mixer
                        mixer_for_timer.borrow_mut().set_num_streams(num_streams);

                        // Create stream for microphone if specified
                        if let Some(mic_id) = mic_source_id {
                            let mixer_clone = Rc::clone(&mixer_for_timer);
                            let is_sink = state.sink_ids.borrow().contains(&mic_id);
                            match create_capture_stream(
                                &core_for_timer,
                                mic_id,
                                is_sink,
                                StreamType::Microphone,
                                mixer_clone,
                                Arc::clone(&state.sample_rate),
                            ) {
                                Ok(stream) => state.streams.push(stream),
                                Err(e) => eprintln!("[Audio] Failed to create mic stream: {}", e),
                            }
                        }

                        // Create stream for system audio if specified
                        if let Some(sys_id) = system_source_id {
                            let mixer_clone = Rc::clone(&mixer_for_timer);
                            let is_sink = state.sink_ids.borrow().contains(&sys_id);
                            match create_capture_stream(
                                &core_for_timer,
                                sys_id,
                                is_sink,
                                StreamType::SystemAudio,
                                mixer_clone,
                                Arc::clone(&state.sample_rate),
                            ) {
                                Ok(stream) => state.streams.push(stream),
                                Err(e) => eprintln!("[Audio] Failed to create sys stream: {}", e),
                            }
                        }
                    }
                    AudioCommand::StopCapture => {
                        eprintln!("[Audio] Stopping capture");
                        state_for_timer.borrow_mut().streams.clear();
                        mixer_for_timer.borrow_mut().set_num_streams(0);
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

/// Type of stream being captured
#[derive(Debug, Clone, Copy)]
enum StreamType {
    Microphone,
    SystemAudio,
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

/// Create a capture stream that sends samples to the mixer.
fn create_capture_stream(
    core: &pipewire::core::Core,
    device_id: u32,
    capture_sink: bool,
    stream_type: StreamType,
    mixer: Rc<RefCell<AudioMixer>>,
    sample_rate: Arc<Mutex<u32>>,
) -> Result<ActiveStream, String> {
    let stream_name = match stream_type {
        StreamType::Microphone => "omnirec-mic-capture",
        StreamType::SystemAudio => "omnirec-system-audio",
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
    let mixer_for_param = Rc::clone(&mixer);
    let mixer_for_process = mixer;

    // Current channels (updated from param_changed)
    let channels: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));
    let channels_for_param = Rc::clone(&channels);
    let channels_for_process = Rc::clone(&channels);

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
                    eprintln!(
                        "[Audio] {:?} stream format: rate={}, channels={}",
                        stream_type, rate, ch
                    );
                    *sample_rate_for_param.lock().unwrap() = rate;
                    *channels_for_param.borrow_mut() = ch;
                    mixer_for_param.borrow_mut().set_channels(ch as u16);
                }
            }
        })
        .state_changed(move |_stream, _user_data, old, new| {
            eprintln!(
                "[Audio] {:?} stream state: {:?} -> {:?}",
                stream_type, old, new
            );
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
                        let ch = *channels_for_process.borrow();

                        // Handle mono to stereo conversion if needed
                        let samples = if ch == 1 {
                            // Duplicate mono samples to stereo
                            samples.iter().flat_map(|&s| [s, s]).collect()
                        } else {
                            samples
                        };

                        // Send to appropriate mixer buffer based on stream type
                        let mut mixer = mixer_for_process.borrow_mut();
                        match stream_type {
                            StreamType::Microphone => mixer.push_mic(&samples),
                            StreamType::SystemAudio => mixer.push_sys(&samples),
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

    Ok(ActiveStream { _stream: stream })
}

// Global audio backend instance (initialized once)
static AUDIO_BACKEND: once_cell::sync::OnceCell<PipeWireAudioBackend> =
    once_cell::sync::OnceCell::new();

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

/// Start audio capture from the specified source (backward compatible).
#[allow(dead_code)]
pub fn start_audio_capture(source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    let backend = get_audio_backend()
        .ok_or_else(|| CaptureError::AudioError("Audio backend not initialized".to_string()))?;
    backend.start_capture(source_id)
}

/// Start audio capture from dual sources with optional AEC.
pub fn start_audio_capture_dual(
    system_source_id: Option<&str>,
    mic_source_id: Option<&str>,
    aec_enabled: bool,
) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    let backend = get_audio_backend()
        .ok_or_else(|| CaptureError::AudioError("Audio backend not initialized".to_string()))?;

    // Set AEC enabled state
    backend.set_aec_enabled(aec_enabled);

    backend.start_capture_dual(system_source_id, mic_source_id)
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
