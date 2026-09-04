//! Build script for rust2genshin-demo.
//!
//! Forces cargo to re-invoke rustc when any of the following changes:
//!   1. Any source file under `src/` (covers `src/lib.rs` and any
//!      submodules declared with `mod foo;`).
//!   2. The rust2genshin backend cdylib (so the .gia is regenerated
//!      with the new backend logic).
//!   3. `target/rust2genshin_demo.gia` — an external rewrite of the
//!      artifact (e.g. another process edits it) invalidates the build
//!      and triggers one extra rebuild to overwrite the external content
//!      with the canonical backend output.
//!
//! Without (2), `cargo build -p rust2genshin-demo` would not invalidate
//! the demo's incremental cache when only the backend's source files
//! change, leaving `target/rust2genshin_demo.gia` stale.
//!
//! # Why (3) doesn't cause an infinite rebuild loop
//!
//! `cargo:rerun-if-changed=<gia>` would normally trigger every build:
//! the backend writes the .gia each run, advancing its mtime, which
//! would then re-trigger the next build. The backend's `save()`
//! (see `core/src/asset/mod.rs::AssetBundle::save`) sidesteps this by
//! comparing the new bytes against the existing file and skipping the
//! write when they match. The mtime only advances when content
//! actually changes — either from a real source change or from an
//! external edit — so the rebuild loop is broken after at most one
//! extra build.

use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};

use std::fmt::format;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir
        .parent()
        .map(|p| p.join("target"))
        .unwrap_or_else(|| PathBuf::from("target"));

    // Track every file under src/ — submodules are picked up via mod foo;
    // and any non-.rs file (e.g. a build-data file) should also retrigger.
    emit_dir_rerun(&manifest_dir.join("src"));

    // Track the backend cdylib. The build-demo pipeline builds the
    // cdylib into `target/debug/` and loads it via -Zcodegen-backend.
    // When its mtime advances, invalidate the demo so rustc is re-run
    // and the backend regenerates the .gia.
    let dll_name = format!("{DLL_PREFIX}rust2genshin{DLL_SUFFIX}");
    let dll_path = target_dir.join("debug").join(dll_name);
    // `cargo:rerun-if-changed` on a missing path is a no-op (cargo treats
    // it as "file does not exist, do not track"). Emit unconditionally so
    // the rerun fires once the file exists.
    println!("cargo:rerun-if-changed={}", dll_path.display());

    // Track the .gia artifact itself. Safe thanks to the backend's
    // content-equal skip-write (see module docs).
    let gia_path = target_dir.join("rust2genshin_demo.gia");
    if gia_path.exists() {
        println!("cargo:rerun-if-changed={}", gia_path.display());
    }
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

