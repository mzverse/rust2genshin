# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

`rust2genshin` compiles a Rust source crate into Genshin Impact node-graph assets (`.gia` files). It does this by implementing a **custom rustc codegen backend** that walks the target crate's MIR after rustc's analysis phase, translates each function into a node-graph (`NodeGraphData`), and writes the result as a protobuf-encoded `.gia` file with a 20-byte header and 4-byte footer.

It does **not** emit machine code — the backend's `link` is essentially a dummy (returns no `CompiledModules`), so the only artifact is the `.gia` written by the backend.

## Toolchain

- **Rust nightly** is required (uses `rustc_private` crates and unstable flags). The intended config is in `rust-toolchain.toml.bak` (this file is **not** auto-loaded; every `cargo` invocation must pass `+nightly` explicitly).
- Required nightly components: `rustc-dev`, `rust-src`, `llvm-tools-preview`.
- System dep: `protoc` (protobuf compiler) for `core/build.rs` to compile `core/proto/asset.proto`.
- Workspace edition `2024`, MSRV `1.100.0`. See root `Cargo.toml`.

## Build & run

The full pipeline is driven by `build-demo/src/main.rs`, which:

1. `cargo +nightly build -p rust2genshin` — builds the backend cdylib (`target/debug/{lib,}rust2genshin{,.so,.dll,.dylib}`).
2. `cargo +nightly clean -p rust2genshin-demo` — invalidates the leaf crate's cache so the backend actually runs.
3. `cargo +nightly rustc --release -p rust2genshin-demo -- -Coverflow-checks=off -Zalways-encode-mir=yes -Zdump-mir=shr -Zcodegen-backend=<backend-path>` — compiles `demo` through the backend.

```shell
cargo +nightly run -p build-demo
```

The output is `target/<crate-name>.gia`.

**Single-step / sub-builds:**

- Backend only: `cargo +nightly build -p rust2genshin`
- A target leaf crate alone (bypassing `build-demo`): `cargo +nightly rustc -p <leaf-crate> -- -Coverflow-checks=off -Zcodegen-backend=$(echo target/debug/librust2genshin*)` (Linux example — DLL suffix varies by OS).
- The CI workflow at `.github/workflows/build.yml` is the canonical reference.

**Tests:** there are no in-repo tests for the backend itself. The leaf crate `demo` is exercised by the `build-demo` flow.

## Workspace layout

This is a Cargo workspace (`Cargo.toml` lists the members):

- `core/` — package **`rust2genshin`** (`crate-type = ["cdylib"]`). The codegen backend itself. Exports `__rustc_codegen_backend` so rustc can load it via `-Zcodegen-backend`. Declares `#![feature(rustc_private)]` and `extern crate rustc_*` to reach into the compiler.
- `lib/` — package **`rust2genshin-lib`** (`#![no_std]`). The user-facing library: types (`Guid`, `Vec3`), `Entity`, `List`, `Dict`, math traits (`Divide`, `I32::shr`, `F32::sqrt/sin/...`), and the `log` helper. Re-exports the `event_listener` proc-macro from `lib-internal`.
- `lib-internal/` — package **`rust2genshin-lib-internal`** (`proc-macro = true`). Defines the proc-macro attributes `native`, `native_calc`, `native_exec`, `event_listener`. These turn fn declarations into bodyless stubs (body replaced with `::core::unreachable!()`) and tag the call site with a numeric ID the backend resolves into a system node.
- `demo/` — package **`rust2genshin-demo`**. The target leaf crate where users write their Rust code. Must be `#![no_std]` (backend warns otherwise).
- `build-demo/` — package **`build-demo`**. Tiny binary that runs the three-step pipeline above. This is what `cargo +nightly run -p build-demo` invokes.

## Backend architecture (`core/src`)

- **`backend.rs`** — Implements `CodegenBackend` (the trait rustc uses to load a custom codegen). `codegen_crate` calls `compile::compile(tcx)` then returns an empty `CompiledModules`. `__rustc_codegen_backend` is the entry symbol.
- **`parser.rs`** — HIR visitors: `dump_hir_to_string`, `collect_entry_points_*`, `collect_entry_point_def_ids`. Recognizes `#[event_listener]` items by inspecting the `outer_expn` of the item span (the attribute is consumed by the proc-macro; only the expansion context remains). This is the **legacy / entry-point** path; the MIR-based path in `compile/` is what currently drives function compilation.
- **`compile/mod.rs`** — The driver. Defines `Compiler`, `Block`, `CompilingFn`, `WithTcx`. `compile(tcx)` creates the output dir, builds the `AssetBundle` (initially empty), collects all `fn` items via HIR walk, calls `touch_fn` on each, and saves `target/<crate>.gia`.
- **`compile/func.rs`** — The MIR→node-graph translator. Per basic block, walks statements (`Assign` Rvalues) then terminators. Implements arithmetic/comparison ops, locals, calls, `SwitchInt` (≤100 cases), `Return`/`Goto`/`Assert`/`Call`/`TailCall`. Calls into `compile_native_call` (see below) for `native*`-tagged functions; otherwise treats callees as composite nodes (recursive `touch_fn`).
- **`compile/native.rs`** — Resolves proc-macro-tagged calls into node-graph nodes. `#[native("divide")]` → `node_divide`; `#[native_calc(N)]` / `#[native_exec(N)]` → `NodeKind::new(N, control_out, control_in, params, ret)`.
- **`compile/native.rs`** also intercepts `LangItem::Panic` (silently emits a `log` warning) and intrinsics like `black_box` (todo).
- **`compile/compile2.rs`**, **`compile/optimize.rs`** — Experimental / WIP passes. `optimize.rs` is currently empty (0 lines).
- **`asset/mod.rs`** — `AssetBundle` (Slab of boxed `Asset`s), `encode()` splits primary vs. dependency assets based on `display: Vec<i64>`, `save()` writes the file with the strict 20-byte header (`[total_size|schema_ver=1|marker=0x0326|file_type=3|proto_len]`) and 4-byte footer (`0x0679`). See comments for the exact byte-level spec.
- **`asset/value.rs`** — TypedValue hierarchy (`AnyValue`, `ValueBool`, `ValueInt`, `ValueFloat`, `ValueString`, `ValueIntList`, `ValueDefault`).
- **`asset/node_graph/`** — Node-graph construction. Submodules by category: `arithmetic`, `comparison`, `control` (switch/branch), `execution` (log, set_local), `query` (`node_local`), `composite` (subgraph calls), `structure`, `trigger`, `client`, `hidden`. Each exports pre-built `NodeKind` constants.
- **`asset/generated/`** — `include!` of `OUT_DIR/rust2genshin.rs` (prost-generated bindings from `proto/asset.proto`).

## Proto schema (`core/proto/asset.proto`)

The `.gia` payload is a serialized `AssetBundleData { assets, dependencies, export_info, mode, engine_version }`, where each `AssetData` carries an `Identifier { source, category, kind, guid, runtime_id }`, a `oneof payload`, and (for graphs) `NodeGraphData { node, port_mapping, comment, blackboard, ... }`. Types in the graph are tagged with `ServerTypeId`/`ClientTypeId` and wrapped in `TypedValue`. The file also embeds rich inline comments explaining the protocol's quirks — read them when in doubt about field semantics.

## How user code becomes a graph
1. User writes `pub fn solve(a: f32, b: f32, c: f32) -> f32 { ... }` in `demo/src/lib.rs`.
2. Marking with `#[unsafe(no_mangle)]` or `#[unsafe(export_name = "...")]` makes it a **composite node** and adds it to `display` (primary assets) — only primary assets are importable by the engine.
3. The backend's HIR walk collects the `LocalDefId`, and `compile_fn` runs:
   - Allocates a `node_local` per MIR local, plus a `set_local` for the return slot.
   - For each `Assign` Rvalue: arithmetic/comparison → matching node; integer division warns (use `.divide()`); right-shift on `i32` is routed through `I32::shr` from `lib` via `find_lib_fn`.
   - For each `Terminator`: `SwitchInt` → `node_switch` (bool → `NODE_IF`, integer → `node_switch` with case list); `Call` to a `native*`-tagged fn → system node; other calls → composite node (recursive).
4. Function-local node refs and connection wires are built up via `Block { begin, end }` and `connect_control` / `set_value_in`.

## Constraints baked into the backend
- `#![no_std]` required on the user crate (compiler warns otherwise).
- Supported primitives: `bool`, `i32`, `isize`, `f32`, `*const str` / `&str` (treated as `ValueString`). Other int/float widths return `compile_ty` errors.
- `i32` arithmetic compiles to unchecked nodes by design (overflow wraps) — keep `-Coverflow-checks=off`.
- `i32 / i32` triggers `span_warn` "Div is slow and big" — use `.divide()` (calls the dedicated divide node).
- `SwitchInt` cases capped at 100 (Genshin editor limit).
- Mirrors of these and unsupported features (events/cast/struct/loops/Box/async/closure/tuple/unsigned/`i64`/client node graph) are listed in `README.md`.

## Common files to edit

| Task | File(s) |
|---|---|
| Add a new system node (e.g. a new trig function) | `lib/src/math.rs` (trait method + `#[native_calc(N)]`), `core/src/compile/native.rs` only if the proc-macro path needs branching |
| Add a new node-graph builder | `core/src/asset/node_graph/<category>.rs` (a `NodeKind` constant), then `pub use` from `core/src/asset/node_graph/mod.rs` |
| Wire up a new Rust MIR construct | `core/src/compile/func.rs` (statements/terminators/Rvalues) |
| Change the protobuf schema | `core/proto/asset.proto` (regenerated into `asset/generated/` by `core/build.rs`) |
| Change `.gia` file framing | `core/src/asset/mod.rs` (`AssetBundle::save`) |
| Add a new proc-macro attribute | `lib-internal/src/lib.rs` |

## Gotchas

- `core/build.rs` calls `prost-build` — requires `protoc` on `PATH`.
- The backend does not produce machine code; the `link` step is essentially a no-op. Crates that need linking (proc-macro deps, executables) are handled by the host rustc; only the leaf crate goes through this backend.
- `cargo check` will NOT trigger the backend — only `cargo build`/`rustc` does (codegen callbacks fire after analysis).
- `find_lib_fn` searches `compiler.lib` (the `rust2genshin-lib` crate) for a specific fully-qualified name; if you add a new helper to `lib/`, you'll get a `Lib fn not found` error pointing to the candidate list until the name match lines up.
- `rust-toolchain.toml` is intentionally absent — the `.bak` file is the template. Don't rename it without ensuring every developer / CI machine has the same nightly, otherwise the codegen ABI breaks silently.