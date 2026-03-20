//! PipeWire-based audio device enumeration for Linux.
//!
//! This module provides audio device discovery using the PipeWire registry.
//! Input devices (microphones) and output devices (sink monitors) are
//! enumerated and kept up-to-date via registry listener callbacks.

use pipewire::{context::Context, main_loop::MainLoop, types::ObjectType};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::capture::error::EnumerationError;
use crate::capture::{AudioSource, AudioSourceType};

/// Handle to the PipeWire audio backend.
///
/// This manages the PipeWire thread and provides device enumeration.
pub struct PipeWireAudioBackend {
    /// Cached input devices (microphones)
    input_devices: Arc<Mutex<Vec<AudioSource>>>,
    /// Cached output devices (sink monitors for system audio)
    output_devices: Arc<Mutex<Vec<AudioSource>>>,
    /// Thread handle
    _thread_handle: JoinHandle<()>,
}

impl PipeWireAudioBackend {
    /// Create and start the PipeWire audio backend.
    pub fn new() -> Result<Self, String> {
        let input_devices = Arc::new(Mutex::new(Vec::new()));
        let output_devices = Arc::new(Mutex::new(Vec::new()));

        let input_devices_clone = Arc::clone(&input_devices);
        let output_devices_clone = Arc::clone(&output_devices);

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_pipewire_audio_thread(input_devices_clone, output_devices_clone) {
                tracing::error!("[Audio] PipeWire thread error: {}", e);
            }
        });

        // Give PipeWire a moment to enumerate devices
        thread::sleep(std::time::Duration::from_millis(200));

        Ok(Self {
            input_devices,
            output_devices,
            _thread_handle: thread_handle,
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
}

/// Run the PipeWire main loop thread for audio device enumeration.
fn run_pipewire_audio_thread(
    input_devices: Arc<Mutex<Vec<AudioSource>>>,
    output_devices: Arc<Mutex<Vec<AudioSource>>>,
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
                        tracing::debug!(
                            "[Audio] Found input device: {} (ID: {})",
                            source.name,
                            source.id
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
                        tracing::debug!(
                            "[Audio] Found output device: {} (ID: {})",
                            source.name,
                            source.id
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
                    tracing::debug!("[Audio] Input device removed: {}", id);
                    let devices: Vec<_> = input_map.borrow().values().cloned().collect();
                    *input_devices.lock().unwrap() = devices;
                }
                if output_map.borrow_mut().remove(&id).is_some() {
                    tracing::debug!("[Audio] Output device removed: {}", id);
                    let devices: Vec<_> = output_map.borrow().values().cloned().collect();
                    *output_devices.lock().unwrap() = devices;
                }
            }
        })
        .register();

    // Run the main loop (blocks until quit) - keeps registry listener active
    mainloop.run();

    Ok(())
}

// Global audio backend instance (initialized once)
static AUDIO_BACKEND: once_cell::sync::OnceCell<PipeWireAudioBackend> =
    once_cell::sync::OnceCell::new();

/// Initialize the global audio backend (call once at app startup).
pub fn init_audio_backend() -> Result<(), String> {
    if AUDIO_BACKEND.get().is_some() {
        tracing::debug!("[Audio] Backend already initialized");
        return Ok(());
    }

    tracing::debug!("[Audio] Initializing PipeWire audio backend...");
    let backend = PipeWireAudioBackend::new()?;
    AUDIO_BACKEND
        .set(backend)
        .map_err(|_| "Audio backend already set")?;
    tracing::debug!("[Audio] PipeWire audio backend initialized");
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
