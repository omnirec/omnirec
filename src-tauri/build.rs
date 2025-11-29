fn main() {
    // Download FFmpeg binary during build (only in release or when explicitly requested)
    // For development, we'll download on first run
    tauri_build::build()
}
