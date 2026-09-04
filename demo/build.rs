//! Build script for rust2genshin-demo.
//!
//! Forces cargo to re-invoke rustc when any of the following changes:
//!   1. Any source file under `src/` (covers `src/lib.rs` and any
//!      submodules declared with `mod foo;`).
//!   2. The rust2genshin backend cdylib (so the .gia is regenerated
//!      with the new backend logic).
//!
//! Without (2), `cargo build -p rust2genshin-demo` would not invalidate
//! the demo's incremental cache when only the backend's source files
//! change, leaving `target/rust2genshin_demo.gia` stale.
//!
//! Note: `target/rust2genshin_demo.gia` is intentionally NOT tracked
//! here. The .gia is a write-only artifact from cargo's perspective;
//! tracking its mtime via cargo:rerun-if-changed would create a loop
//! because every successful build advances the .gia's mtime, which
//! would then re-trigger the next build. External edits to the .gia
//! are not auto-detected — run `cargo +nightly clean -p
//! rust2genshin-demo && cargo +nightly run -p build-demo` to force
//! regeneration.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Track every file under src/ — submodules are picked up via mod foo;
    // and any non-.rs file (e.g. a build-data file) should also retrigger.
    emit_dir_rerun(&manifest_dir.join("src"));

    // Track the backend cdylib. The build-demo pipeline builds the
    // cdylib into `target/debug/` and loads it via -Zcodegen-backend.
    // When its mtime advances, invalidate the demo so rustc is re-run
    // and the backend regenerates the .gia.
    let dll_name = if cfg!(windows) {
        "rust2genshin.dll"
    } else if cfg!(target_os = "macos") {
        "librust2genshin.dylib"
    } else {
        "librust2genshin.so"
    };
    let target_dir = manifest_dir
        .parent()
        .map(|p| p.join("target"))
        .unwrap_or_else(|| PathBuf::from("target"));
    let dll_path = target_dir.join("debug").join(dll_name);
    // `cargo:rerun-if-changed` on a missing path is a no-op (cargo treats
    // it as "file does not exist, do not track"). Emit unconditionally so
    // the rerun fires once the file exists.
    println!("cargo:rerun-if-changed={}", dll_path.display());
}

fn emit_dir_rerun(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return, // src/ might not exist; nothing to track
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_dir_rerun(&path);
        } else {
            // cargo:rerun-if-changed on every file in src/. Files included
            // via `mod foo;` are .rs files; .toml/.md etc. are inert but
            // harmless to track.
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

