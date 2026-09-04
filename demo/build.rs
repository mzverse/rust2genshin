//! Build script for rust2genshin-demo.
//!
//! Forces cargo to re-invoke rustc when:
//!   1. The demo's source (`src/lib.rs`) changes.
//!   2. The rust2genshin backend cdylib is rebuilt (so the .gia is
//!      regenerated with the new backend logic).
//!
//! Without (2), `cargo build -p rust2genshin-demo` would not invalidate
//! the demo's incremental cache when only the backend's source files
//! change, leaving `target/rust2genshin_demo.gia` stale.

use std::path::PathBuf;

fn main() {
    // Always re-run when the demo source changes.
    println!("cargo:rerun-if-changed=src/lib.rs");

    // Track the backend cdylib. The build-demo pipeline builds the
    // cdylib into `target/debug/` and loads it via -Zcodegen-backend.
    // When its mtime advances, invalidate the demo so rustc is re-run
    // and the backend regenerates the .gia.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dll_name = if cfg!(windows) {
        "rust2genshin.dll"
    } else if cfg!(target_os = "macos") {
        "librust2genshin.dylib"
    } else {
        "librust2genshin.so"
    };
    let dll_path = PathBuf::from(manifest_dir)
        .join("..")
        .join("target")
        .join("debug")
        .join(dll_name);
    if dll_path.exists() {
        // `cargo:rerun-if-changed` on a missing path is silently a no-op
        // (cargo treats it as "file does not exist, do not track"). Emit
        // unconditionally so the rerun fires once the file exists.
        println!("cargo:rerun-if-changed={}", dll_path.display());
    }
}
