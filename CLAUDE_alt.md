# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

`rust2genshin` compiles a Rust source crate into Genshin Impact node-graph assets (`.gia` files). It does this by implementing a **custom rustc codegen backend** that walks the target crate's MIR after rustc's analysis phase, translates each function into a node-graph (`NodeGraphData`), and writes the result as a protobuf-encoded `.gia` file with a 20-byte header and 4-byte footer.

It does **not** emit machine code — the backend's `link` is essentially a dummy (returns no `CompiledModules`), so the only artifact is the `.gia` written by the backend.

## Toolchain

- **Rust nightly** is required (uses `rustc_private` crates and unstable flags). The config lives in `rust-toolchain.toml` at the repo root, which **is** auto-loaded by cargo/rustup, so plain `cargo …` already uses the pinned toolchain — there is no need to pass `+nightly` (or any toolchain override) explicitly. There is no `rust-toolchain.toml.bak`.
  - `rust-toolchain.toml`: `channel = "nightly-2026-10-08"`, `components = ["rustc-dev", "rust-src", "llvm-tools-preview"]`.
  - Pinning matters: the codegen ABI breaks silently on a different nightly.
- System dep: `protoc` (protobuf compiler) for `core/build.rs` to compile `core/proto/asset.proto`. CI installs it with `sudo apt install protobuf-compiler` (`.github/workflows/build.yml:20`).
- Workspace edition `2024` (root `Cargo.toml:28`, `workspace.package`). The workspace does **not** declare a `rust-version` key anywhere, so there is no MSRV claim to rely on.

## Build & run

The full pipeline is driven by `build-demo` (`build-demo/src/build.rs`), which:

1. `cargo build -p rust2genshin` (`build-demo/src/build.rs:40`) — builds the backend cdylib (`target/debug/{lib,}rust2genshin{,.so,.dll,.dylib}`).
2. `gen_target` (`build-demo/src/build.rs:32-37`) — renders `genshin-unknown-<target>.json.template` with the absolute path of the built backend into `genshin-unknown-<target>.json`; the backend is selected by that spec's `default-codegen-backend`.
3. `cargo build --release -p rust2genshin-demo --target ./<target>.json -Zunstable-options -Zjson-target-spec -Zbuild-std=core` (`build-demo/src/build.rs:51-60`) — compiles `demo` through the backend.

There is **no** separate `cargo clean` step in the script any more (it is commented out at `build-demo/src/build.rs:44-45`), and **no `-Zcodegen-backend=` flag is passed** — the custom target spec carries the backend instead.

`build-demo` has **two binaries** — `beyond-server` and `classic-server` (`build-demo/Cargo.toml:7-13`) — so `--bin` is mandatory:

```shell
cargo run -p build-demo --bin beyond-server
cargo run -p build-demo --bin classic-server
```

The output is `target/<target-name>/release/<crate-name>.gia`, e.g. `target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia` (`target/` + target-spec name + `/release/` + crate name).

**Single-step / sub-builds:**

- Backend only: `cargo build -p rust2genshin`
- The viewer is not a workspace default member (`Cargo.toml:17-23`, because `gpui` needs X11/Wayland/font system libs that CI does not install) — build it explicitly with `cargo build -p rust2genshin-viewer`.
- The playground backend is likewise not a default member. Start the service with `cargo run -p rust2genshin-playground` — it requires `docker` and `git` on `PATH` and exits at startup if either is missing. See the doc-site page on the playground for the config table, webhook setup, and disk accounting.
- The CI workflow at `.github/workflows/build.yml` is the canonical reference; it runs exactly `cargo run -p build-demo --bin beyond-server` (`.github/workflows/build.yml:22`).

**Tests:** there are no in-repo tests for the backend itself. The leaf crate `demo` is exercised by the `build-demo` flow.

## Workspace layout

This is a Cargo workspace (`Cargo.toml` lists the members and `default-members`; `viewer` is a member but deliberately excluded from `default-members`):

```
core/                package `rust2genshin` (cdylib `rust2genshin` + bin `rust2genshin-linker`)
  build.rs           prost-build writes the proto bindings into OUT_DIR
  proto/asset.proto  the single .gia protobuf layer
  src/               asset.rs / value.rs / structure.rs / linker.rs / lib.rs / compile/ / node/
lib/                 package `rust2genshin-lib` (#![no_std], the target crate's runtime API)
  src/               asynchronous / boxed / dict / entity / list / math / player / sc / task + event/
lib-internal/        package `rust2genshin-lib-internal` (proc-macro)
demo/                package `rust2genshin-demo` — the target leaf crate, node graphs live here
build-demo/          package `build-demo` (bins: beyond-server / classic-server)
viewer/              package `rust2genshin-viewer`, reads the intermediate `.ogia` (not a default member)
playground/          package `rust2genshin-playground` — backend for the doc-site "playground" page
                     (WebSocket + a throwaway docker container running `cargo build`; not a default member)
docs/                doc-site builder + `docs/typst/` sources
```

- `core/` — the codegen backend itself. Exports `__rustc_codegen_backend` so rustc can load it. Declares `#![feature(rustc_private)]` and `extern crate rustc_*` to reach into the compiler.
- `lib/` — the user-facing library: types, `Entity`/`Player`, containers, math traits (`Divide`, `I32::shr`, `F32::sqrt/sin/...`), and the `log` helper. Re-exports the proc-macros from `lib-internal`.
- `lib-internal/` — `proc-macro = true`, all macros in one `src/lib.rs`: `native` (`:12`), `native_calc` (`:38`), `native_exec` (`:43`), `native_enum` (`:48`), `event` (`:68`), `event_listener` (`:73`), `async_loop` (`:90`), `asynchronous` (`:106`). They turn fn declarations into bodyless stubs and tag the call site with a numeric ID the backend resolves into a system node.
- `demo/` — the target leaf crate where users write their Rust code. Must be `#![no_std]` (the backend warns otherwise but still compiles).

## Backend architecture (`core/src`)

- **`compile/backend.rs`** — Implements `CodegenBackend`. `codegen_crate` (`:60`) calls `compile::compile(tcx)` (`:70`) and wraps the result in a `CompiledModule` whose object is the `.ogia`; `link` (`:85`) delegates to rustc's shared `link_binary` (`:103`). `__rustc_codegen_backend` is the entry symbol. The file header comment states the "no machine code" contract.
- **`compile/mod.rs`** — The driver. `compile(tcx)` (`:56`) warns when the crate is not `#![no_std]` (`:70`; only proc-macro / test crates are actually skipped), then builds `Compiler::new(tcx)` (`:145`, which resolves the `rust2genshin_lib` dependency via `LIB_NAME` at `:132`) and runs it. `Compiler::run` (`:168`) seeds the main graph and takes entry points from `exported_generic_symbols` chained with `exported_non_generic_symbols` (`:170`). Also defines `Block { begin, end }` (`:93`) and `find_lib_fn` (`:117`).
- **`compile/func.rs`** — The MIR->node-graph translator (`CompilingFn`, `FnDecl`). Per basic block it walks statements (`Assign` Rvalues) then terminators, calling into `compile/native.rs` for `native*`-tagged functions and otherwise treating callees as composite nodes (recursive `touch_fn`, `compile/mod.rs:237`). `Assert` and `panic` terminators are skipped outright, so overflow and bounds checks never reach the graph.
- **`compile/place.rs`** — `CompiledLocal` / `CompilingLocals`: recursively flattens MIR `Place` projections (tuple fields, struct fields, derefs) into node-graph locals; closures therefore end up as a `#closure + args` pair.
- **`compile/ty.rs`** — `compile_ty` plus type mangling. Primitives map to `NativeKind` (`ty.rs:191-224`): `bool` -> `Bool`; `isize`/`i32`/`u32`/`usize` -> `Int`; `f32` -> `Float`; `str` and `*const str` -> `String`; raw pointers to slices/arrays -> `List`. Other int/float widths (`i8`/`i16`/`i64`/`i128`, `f16`/`f64`/`f128`) are hard errors.
- **`compile/ir.rs`** + **`compile/optimize.rs`** — `NodeGraphIr` and `Optimizer`. `ir.rs` holds the IR data structures plus `Link`/`ValueIn` lowering helpers; `optimize.rs` (852 lines, **not** empty) holds the actual `impl Optimizer` passes: `verify` (`:24`), `link_node` (`:30`), `lower` (`:85`), `optimize` (`:363`) and the elimination family (`eliminate_solos`, `eliminate_nop`, `raise_if`, `eliminate_branches`, `eliminate_switch`, `eliminate_set_local`, `eliminate_local`, `eliminate_calc`, `eliminate_assemble`, `eliminate_destructure`, `eliminate_bool_eq_int`, `eliminate_not_not`).
- **`compile/link.rs`** — `Linker` and `Target`. `Linker::link` (`:67`) walks the target's functions, calls `set_primary` for the exported ones (`:74`), lowers and `layout`s the main graph (`:81-82`), then `assets.save` (`:89`). `Linker::touch_fn` (`:95`) turns each compiled function into its own composite-node asset.
- **`asset.rs`** — `AssetBundle` (primary + dependency assets) and `AssetBundle::save` (`:109`), which writes the strict 20-byte header (`[total_size | schema_ver=1 | marker=0x0326 | file_type=3 | proto_len]`) and the 4-byte footer (`0x0679`), skipping the write when the bytes are unchanged so the output mtime stays stable. The prost bindings are `include!`d from `OUT_DIR/rust2genshin.rs` (`:8`), generated by `core/build.rs` into `OUT_DIR` — **not** into a checked-in `asset/generated/` directory.
- **`value.rs`** — the node-graph type system: `NativeKind` (the enum) and `NativeValue` (the trait each type implements). The old `AnyValue` / `ValueBool` / `ValueInt` / `ValueString` hierarchy no longer exists.
- **`node/`** — node construction, split by domain: `mod.rs` (`NodeKind`, `Node`, `Connection`, `Link`, `NodeRef`; `NODE_ID_BEGIN = 1`), `arithmetic.rs`, `control.rs` (switch/branch), `execution.rs` (log, set_local), `query.rs` (`node_local`), `composite.rs` (subgraph calls), `decl.rs` (pin declarations), `hidden.rs`, `layout.rs`, `client.rs`. Nodes are built at runtime; there is no registry table.
- **`compile/native.rs`** — Resolves proc-macro-tagged calls into node-graph nodes. `#[native("divide")]` → `node_divide`; `#[native_calc(N)]` / `#[native_exec(N)]` → `NodeKind::new(N, control_out, control_in, params, ret)`. It also intercepts `LangItem::Panic` (emits a `log` warning) and intrinsics like `black_box`.

There is no `compile2.rs`, no `asset/mod.rs`, no `asset/value.rs`, no `asset/node_graph/` directory, and no checked-in `asset/generated/`. Struct-related nodes live in `core/src/structure.rs`; the graph-layout pass lives in `core/src/node/layout.rs`.

## Proto schema (`core/proto/asset.proto`)

The `.gia` payload is a serialized `AssetBundleData { assets, dependencies, export_info, mode, engine_version }`, where each `AssetData` carries an `Identifier { source, category, kind, guid, runtime_id }`, a `oneof payload`, and (for graphs) `NodeGraphData { node, port_mapping, comment, blackboard, ... }`. Types in the graph are tagged with `ServerTypeId`/`ClientTypeId` and wrapped in `TypedValue`. The file also embeds rich inline comments explaining the protocol's quirks — read them when in doubt about field semantics.

## How user code becomes a graph
1. User writes `pub fn solve(a: f32, b: f32, c: f32) -> f32 { ... }` in `demo/src/lib.rs`.
2. Marking it with `#[unsafe(no_mangle)]` / `#[unsafe(export_name = "...")]` puts it into `exported_generic_symbols` / `exported_non_generic_symbols`, which is exactly how `Compiler::run` picks entry points (`compile/mod.rs:170`) — there is no HIR walk for entry-point discovery any more. Exported functions also become **primary assets** in `Linker::link` (`compile/link.rs:72-74`) — only primary assets are importable by the engine.
3. For each entry point the compiler first tries `compile_native_call` (a `#[native*]`-tagged fn compiles straight to a system node); anything else goes through `touch_fn` (`compile/mod.rs:237`), which recurses into callees.
4. `compile_fn` (`compile/mod.rs:248`) then builds the function's IR:
   - MIR locals become node-graph locals via `compile/place.rs` (`CompiledLocal` / `CompilingLocals`), plus a `set_local` for the return slot.
   - Each `Rvalue::BinaryOp` maps to a node from `core/src/node/arithmetic.rs` (`func.rs:141-162`): add/sub/mul, `Div` → `node_divide` (there is no longer a "Div is slow and big" warning — integer division compiles straight to the divide node), `Rem`, bitwise ops, comparisons, `Shl` → `NODE_LEFT_SHIFT`. `Shr`/`ShrUnchecked` and `Cmp`/`Offset` are still `todo!()` (`func.rs:155`, `:161`). The `*Unchecked` / `*WithOverflow` variants are collapsed onto the same plain nodes, which is why overflow wraps by design.
   - Each `Terminator`: `SwitchInt` → `node_switch` (bool → `NODE_IF`, integer → `node_switch` with a case list, `func.rs:484-489`); `Call` to a `native*`-tagged fn → system node; other calls → composite node (recursive). `Return`/`Goto` drive control flow; `Assert`/`panic` are dropped.
5. Function-local node refs and wires are tracked by `Block { begin, end }` (`compile/mod.rs:93`) plus `link_control` / `set_value_in`.
6. `Compiler::run` finishes by handing the `Target` to the linker; `Linker::link` (`compile/link.rs:67`) marks exported graphs primary, calls `Optimizer::lower` (`compile/optimize.rs:85`, which runs `optimize()` to a fixpoint at `:363`), runs `node::layout::layout`, and finally `AssetBundle::save` (`core/src/asset.rs:109`) writes the `.gia`.
7. Separately, `Compiler::save` (`compile/mod.rs:161`) writes the intermediate `.ogia` — a bincode-serialized `Target` — which is what the viewer reads; it is not the shipping artifact.

## Constraints baked into the backend
- `#![no_std]` expected on the user crate (the backend warns otherwise but still compiles it — `compile/mod.rs:70`).
- Supported primitives, per `compile_ty` (`core/src/compile/ty.rs:191-224`): `bool`, `i32`, `isize`, `u32`, `usize`, `f32`, `*const str` / `&str` (as `NativeKind::String`). Other int widths (`i8`/`i16`/`i64`/`i128`) and float widths (`f16`/`f64`/`f128`) are `span_err`s. Handle types (`Guid`, `Entity`, `Prefab`, `Config`, `Faction`) are accepted as opaque `NativeKind`s (`ty.rs:29-33`).
- `i32` arithmetic compiles to **unchecked** nodes by design (overflow wraps) — this is why you do not want overflow checks in the graph.
- You therefore do **not** need to pass `-Coverflow-checks=off`. Nothing in the tooling passes it: `build-demo/src/build.rs` does not (`cargo clean` lines are commented out at `:44-45`, and the only `RUSTFLAGS` mention is a commented-out `-Zalways-encode-mir` line at `:48`), and CI does not either. The workspace relies on `[profile.release] overflow-checks = false` (`Cargo.toml:43`) plus the fact that `Assert` terminators are dropped during translation anyway — the workspace documentation page on debugging explains the history.
- There is no "Div is slow and big" warning any more; `BinOp::Div` maps directly to `node_divide` (`core/src/compile/func.rs:148`).
- `Shr`/`ShrUnchecked` and `Cmp`/`Offset` are still `todo!()` and will panic the backend if you write them.
- Mirrors of these and unsupported features (events/cast/struct/loops/Box/async/closure/tuple/unsigned/`i64`/client node graph) are listed in `README.md`.

## Common files to edit

| Task | File(s) |
|---|---|
| Add a new system node (e.g. a new trig function) | `lib/src/math.rs` (trait method + `#[native_calc(N)]`), `core/src/compile/native.rs` only if the proc-macro path needs branching |
| Add a new node-graph builder | `core/src/node/<category>.rs` (a constructor returning a `NodeKind`), then wire it at the use site in `core/src/compile/func.rs` or `core/src/compile/ir.rs` |
| Add a new value type | `core/src/value.rs` — a `NativeKind` variant plus `impl NativeValue` |
| Wire up a new Rust MIR construct | `core/src/compile/func.rs` (statements / terminators / Rvalues) |
| Add an IR/graph optimization pass | `core/src/compile/optimize.rs` (`impl Optimizer`, alongside `eliminate_*`) |
| Change the protobuf schema | `core/proto/asset.proto` — prost regenerates into `OUT_DIR` on the next `cargo build`; the result is `include!`d at `core/src/asset.rs:8` |
| Change `.gia` file framing | `core/src/asset.rs` (`AssetBundle::save`) |
| Add a new proc-macro attribute | `lib-internal/src/lib.rs` |

## Gotchas

- `core/build.rs` calls `prost-build` — requires `protoc` on `PATH`.
- The backend does not produce machine code; the `link` step is essentially a no-op. Crates that need linking (proc-macro deps, executables) are handled by the host rustc; only the leaf crate goes through this backend.
- `cargo check` will NOT trigger the backend — only `cargo build`/`rustc` does (codegen callbacks fire after analysis).
- `find_lib_fn` searches `compiler.lib` (the `rust2genshin-lib` crate) for a specific fully-qualified name; if you add a new helper to `lib/`, you'll get a `Lib fn not found` error pointing to the candidate list until the name match lines up.
- `rust-toolchain.toml` **is** present at the repo root and pins `nightly-2026-10-08` with `rustc-dev`, `rust-src`, and `llvm-tools-preview`; rustup auto-loads it, so plain `cargo` picks it up. Don't relax or delete it without ensuring every developer / CI machine has the same nightly, otherwise the codegen ABI breaks silently. There is no `.bak` template.
- `build-demo` has two binaries (`beyond-server`, `classic-server`) — always pass `--bin`, or `cargo run -p build-demo` will fail with an ambiguity error.
- Never add `-Zcodegen-backend=<path>` to the build: `build-demo` selects the backend through the generated `genshin-unknown-<target>.json` spec's `default-codegen-backend` field, and passes `-Zjson-target-spec` / `-Zbuild-std=core` instead.
- Do not manually pass `-Coverflow-checks=off` or `-Zalways-encode-mir=yes`; nothing in the build script or CI passes them. The reason overflow checks are unwanted is a design property, not a build flag: `i32` arithmetic is lowered to unchecked nodes (`core/src/compile/func.rs:142-147` folds `AddUnchecked` / `AddWithOverflow` onto the plain `node_add`), and `Assert` terminators are dropped during translation, so overflow wraps in the graph by design.