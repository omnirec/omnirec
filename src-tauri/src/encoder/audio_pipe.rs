//! Cross-platform named pipe for streaming raw PCM audio to FFmpeg.
//!
//! FFmpeg accepts only one stdin (`-i -`). To provide both video and audio to
//! a single FFmpeg process, audio is written to a named pipe that FFmpeg reads
//! as a second input (`-f s16le -ar 48000 -ac 1 -i <pipe_path>`).

/// A writable named pipe for streaming audio data to FFmpeg.
pub struct AudioPipe {
    /// The pipe path that should be passed to FFmpeg as an input.
    path: String,
    /// The write handle to the pipe.
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
    #[cfg(unix)]
    file: Option<std::fs::File>,
}

// Safety: The Windows HANDLE is only accessed from the thread that owns AudioPipe.
unsafe impl Send for AudioPipe {}

impl AudioPipe {
    /// Create a new named pipe and return the `AudioPipe`.
    ///
    /// On Windows, creates a `\\.\pipe\omnirec_audio_<pid>` pipe.
    /// On Unix, creates a FIFO at `/tmp/omnirec_audio_<pid>`.
    ///
    /// The pipe is created but not yet opened for writing. Call [`open`] after
    /// FFmpeg has been spawned (FFmpeg blocks on the pipe until a writer connects).
    pub fn create() -> Result<Self, String> {
        let pid = std::process::id();

        #[cfg(windows)]
        {
            Self::create_windows(pid)
        }

        #[cfg(unix)]
        {
            Self::create_unix(pid)
        }
    }

    /// Get the pipe path to pass to FFmpeg as an input.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Open the pipe for writing. This unblocks FFmpeg which is waiting for a
    /// writer to connect to the pipe.
    pub fn open(&mut self) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.open_windows()
        }

        #[cfg(unix)]
        {
            self.open_unix()
        }
    }

    /// Write raw bytes to the pipe.
    pub fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.write_windows(data)
        }

        #[cfg(unix)]
        {
            use std::io::Write;
            if let Some(ref mut file) = self.file {
                file.write_all(data)
                    .map_err(|e| format!("Failed to write to audio pipe: {}", e))
            } else {
                Err("Audio pipe not open".to_string())
            }
        }
    }

    /// Write silence (zero bytes) for the given number of mono s16le samples.
    pub fn write_silence(&mut self, num_samples: usize) -> Result<(), String> {
        // s16le mono: 2 bytes per sample, all zeros = silence
        let silence = vec![0u8; num_samples * 2];
        self.write_all(&silence)
    }
}

/// Convert f32 mono samples to s16le (signed 16-bit little-endian) bytes.
pub fn f32_mono_to_s16le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * 32767.0) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

// ── Windows implementation ──────────────────────────────────────────────────

#[cfg(windows)]
impl AudioPipe {
    fn create_windows(pid: u32) -> Result<Self, String> {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
        use windows::Win32::System::Pipes::{CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};

        let path = format!(r"\\.\pipe\omnirec_audio_{}", pid);
        let h_path = HSTRING::from(&path);

        // PIPE_ACCESS_OUTBOUND = 0x00000002
        let pipe_access_outbound = FILE_FLAGS_AND_ATTRIBUTES(0x00000002);

        let handle = unsafe {
            CreateNamedPipeW(
                &h_path,
                pipe_access_outbound,
                PIPE_TYPE_BYTE | PIPE_WAIT,
                1,                // max instances
                64 * 1024 * 1024, // out buffer size (64 MB, per design doc)
                0,                // in buffer size
                0,                // default timeout
                None,             // security attributes
            )
        };

        if handle == INVALID_HANDLE_VALUE || handle.is_invalid() {
            return Err(format!(
                "Failed to create named pipe: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(Self { path, handle })
    }

    fn open_windows(&mut self) -> Result<(), String> {
        use windows::Win32::System::Pipes::ConnectNamedPipe;

        // ConnectNamedPipe blocks until FFmpeg opens the pipe for reading.
        unsafe { ConnectNamedPipe(self.handle, None) }.or_else(|e| {
            // ERROR_PIPE_CONNECTED (535) means FFmpeg connected before we called
            // ConnectNamedPipe, which is fine.
            if e.code().0 as u32 == 0x80070217 {
                // HRESULT for ERROR_PIPE_CONNECTED
                Ok(())
            } else {
                Err(format!("ConnectNamedPipe failed: {}", e))
            }
        })
    }

    fn write_windows(&mut self, data: &[u8]) -> Result<(), String> {
        use windows::Win32::Storage::FileSystem::WriteFile;

        let mut total_written: usize = 0;
        while total_written < data.len() {
            let remaining = &data[total_written..];
            let mut bytes_written: u32 = 0;
            unsafe { WriteFile(self.handle, Some(remaining), Some(&mut bytes_written), None) }
                .map_err(|e| format!("Failed to write to audio pipe: {}", e))?;
            total_written += bytes_written as usize;
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for AudioPipe {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Pipes::DisconnectNamedPipe;

        unsafe {
            let _ = DisconnectNamedPipe(self.handle);
            let _ = CloseHandle(self.handle);
        }
    }
}

// ── Unix implementation ─────────────────────────────────────────────────────

#[cfg(unix)]
impl AudioPipe {
    fn create_unix(pid: u32) -> Result<Self, String> {
        let path = format!("/tmp/omnirec_audio_{}", pid);

        // Remove stale FIFO if it exists
        let _ = std::fs::remove_file(&path);

        // Create the FIFO
        nix::unistd::mkfifo(path.as_str(), nix::sys::stat::Mode::S_IRWXU)
            .map_err(|e| format!("Failed to create FIFO: {}", e))?;

        Ok(Self { path, file: None })
    }

    fn open_unix(&mut self) -> Result<(), String> {
        // Opening a FIFO for writing blocks until a reader (FFmpeg) opens it.
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .map_err(|e| format!("Failed to open FIFO for writing: {}", e))?;
        self.file = Some(file);
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for AudioPipe {
    fn drop(&mut self) {
        self.file = None; // Close the file handle first
        let _ = std::fs::remove_file(&self.path);
    }
}
