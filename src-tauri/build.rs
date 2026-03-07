fn main() {
    // On macOS: copy libwhisper.dylib from vtx-engine's whisper-cache into
    // src-tauri/lib/ so tauri-build can bundle it as a Frameworks resource.
    // vtx-engine's build script downloads it to target/whisper-cache/ before
    // this script runs (dependency build scripts run first).
    #[cfg(target_os = "macos")]
    stage_libwhisper();

    tauri_build::build()
}

#[cfg(target_os = "macos")]
fn stage_libwhisper() {
    use std::path::PathBuf;

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Walk up from OUT_DIR to find the workspace target/ directory.
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "target").unwrap_or(false));

    let Some(target_dir) = target_dir else {
        println!("cargo:warning=build.rs: could not locate target/ dir; skipping libwhisper.dylib staging");
        return;
    };

    let src = target_dir.join("whisper-cache").join("libwhisper.dylib");
    let lib_dir = PathBuf::from("lib");
    let dest = lib_dir.join("libwhisper.dylib");

    if !src.exists() {
        // Not yet downloaded (e.g. stub created by make stub-sidecar for lint).
        // tauri-build will still see the stub or real file placed by a prior step.
        println!(
            "cargo:warning=build.rs: {} not found; skipping copy (stub or real file must already exist at {})",
            src.display(),
            dest.display()
        );
        return;
    }

    if let Err(e) = std::fs::create_dir_all(&lib_dir) {
        println!("cargo:warning=build.rs: failed to create lib/: {}", e);
        return;
    }

    // Only overwrite if the destination is a zero-byte stub or doesn't exist.
    let dest_size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    if dest_size == 0 {
        if let Err(e) = std::fs::copy(&src, &dest) {
            println!(
                "cargo:warning=build.rs: failed to copy libwhisper.dylib: {}",
                e
            );
        } else {
            println!(
                "cargo:warning=build.rs: staged libwhisper.dylib to {}",
                dest.display()
            );
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=lib/libwhisper.dylib");
}
