# Clippy Cleanup — Design Spec

**Date:** 2026-09-05
**Status:** Approved (brainstorming complete)
**Scope:** Resolve all 127 clippy warnings across the workspace via `cargo clippy --fix` for auto-fixable lints, then hand-fix the remaining judgment-call lints. Verify `cargo +nightly clippy --workspace --all-targets` is clean and the `.gia` output SHA-256 is unchanged.

## Context

The workspace has accumulated 127 clippy warnings across `core/`, `lib/`, and `lib-internal/`. Most are mechanical (derivable `Default` impls, redundant `Some(x).ok()`, unused imports) and have machine-applicable fixes via `cargo clippy --fix`. A few require judgment: one `diverging_sub_expression` (where `todo!() as Result<_>` makes the cast unreachable), one `large_enum_variant` (suggesting boxing), two `missing_safety_doc` (on unsafe traits in `lib/src/math.rs`), and ~50 `unused_variable` warnings mostly in WIP files (`compile2.rs`, `optimize.rs`).

This is a **pure cleanup pass** — no behavior changes. The user's previous preference is to keep TODOs on unimplemented features and not remove commented debug code. We honor that by **prefixing** unused variables with `_` rather than deleting the lines (the WIP structure stays as-is), and by keeping any `// TODO` comments that aren't on the auto-fix path.

The verification gate is the same one used by the inline-polish sub-project: `target/rust2genshin_demo.gia` SHA-256 must remain `f3bc21afee2dc209664a760c8c79fd34b35292b098de44da09c79ac82d18e584` after all fixes.

## Approach

Five sequential phases. Each phase is committed independently. After each phase, run `cargo +nightly build -p rust2genshin` and `cargo +nightly test -p rust2genshin` to confirm nothing broke. The SHA-256 gate runs after Phase 5.

### Phase 1 — Auto-fix via `cargo clippy --fix`

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets --fix --allow-dirty --allow-no-vcs
```

This applies machine-applicable fixes for:
- `derivable_impls` (21) — replace manual `impl Default for X { fn default() -> Self { Self(...) } }` with `#[derive(Default)]` on the struct
- `needless_late_init` (14) — move variable initialization to the declaration site
- `match_result_ok` (6) — `Some(x).ok()` → `Some(x)`
- `collapsible_if` (2) — combine nested `if` blocks with `&&`
- `get_first` (1) — `data.get(0)` → `data.first()`
- `new_without_default` (1) — add `Default` impl to `NodeGraphComposite`
- `clone_on_copy` (1) — remove `.clone()` on Copy types
- `manual_map` (1) — refactor to `Option::map`
- Many unused imports (auto-removed)

`cargo clippy --fix` will **not** apply (these require hand-fixing): unused variable `_`-prefixing (no auto-applicable suggestion), the 3 judgment lints (`diverging_sub_expression`, `large_enum_variant`, `missing_safety_doc`).

After Phase 1, run clippy again to get a fresh warning list and confirm what remains.

Commit:
```bash
cd F:/rust2genshin && git add -A && git commit -m "refactor: apply cargo clippy --fix for auto-fixable warnings"
```

### Phase 2 — Hand-fix `diverging_sub_expression` at `core/src/compile/native.rs:22`

Current code (around line 22, inside `compile_native_call`):
```rust
"black_box" => todo!(),
other => todo!("intrinsic: {other}") as Result<_>,
```

Clippy's fix: drop the `as Result<_>` cast — it's unreachable since `todo!()` diverges. The match arm's `!` type coerces to the expected match-arm type `Result<NodeKind>`.

Hand-fix to:
```rust
"black_box" => todo!(),
other => todo!("intrinsic: {other}"),
```

If the match expression's type inference breaks (because the first arm's `todo!()` is now also `!` and the function expects `Result<NodeKind>`), wrap the match in an explicit annotation: `let result: Result<NodeKind> = match ... { ... }; result.into()` — but in practice Rust's match-arm divergence propagation handles it.

Commit:
```bash
cd F:/rust2genshin && git add core/src/compile/native.rs && git commit -m "fix(core): drop unreachable 'as Result<_>' cast in intrinsic match"
```

### Phase 3 — Hand-fix `large_enum_variant` (location TBD)

Clippy's suggestion (from output):
```
help: consider boxing the large fields or introducing indirection in some other way
   --> core/src/asset/node_graph/...
    |
    -         InterfaceData(super::NodeInterfaceContainer),
    +         InterfaceData(Box<super::NodeInterfaceContainer>),
```

Locate the exact file/line via `cargo +nightly clippy --workspace --all-targets 2>&1 | grep -B 2 -A 6 large_enum_variant`. Apply the suggestion as-is: change `InterfaceData(super::NodeInterfaceContainer)` → `InterfaceData(Box<super::NodeInterfaceContainer>)`. Update all match arms and constructors accordingly.

**Caveat:** boxing changes the enum's wire size. Since `AssetData` (the parent enum) is serialized to protobuf via prost, and `Box<T>` serializes via `Deref` to `T`, the protobuf output should be byte-identical. But verify with the SHA-256 gate.

Commit:
```bash
cd F:/rust2genshin && git add core/src/asset/node_graph/ && git commit -m "refactor(core): box large InterfaceData variant per clippy suggestion"
```

### Phase 4 — Hand-fix `missing_safety_doc` for `unsafe trait I32` and `unsafe trait F32`

Both traits are at `lib/src/math.rs`. The unsafe-trait pattern: each trait is implemented for one specific primitive type via proc-macro (`#[native_calc(N)]` attributes), and the safety invariant is that the proc-macro and the kernel IDs line up.

Current:
```rust
pub unsafe trait I32 {
    fn ushr(self, rhs: Self) -> Self;
    fn shr(self, rhs: Self) -> Self;
}

pub unsafe trait F32 {
    fn sqrt(self) -> Self;
    fn log(self, base: Self) -> Self;
    // ... etc.
}
```

Hand-fix to add `# Safety` sections:
```rust
/// Integer math helpers backed by genshin node-graph kernel operations.
///
/// # Safety
///
/// This trait must only be implemented for primitive integer types whose
/// genshin node-graph kernel IDs match those in the `#[native_calc(N)]`
/// attributes on each method. The current implementation targets `i32` and
/// uses kernels `779` (ushr) and the inlined shr logic. Implementing this
/// trait for any other type would dispatch to wrong kernel IDs and produce
/// a corrupt node graph.
pub unsafe trait I32 {
    fn ushr(self, rhs: Self) -> Self;
    fn shr(self, rhs: Self) -> Self;
}

/// Float math helpers backed by genshin node-graph kernel operations.
///
/// # Safety
///
/// This trait must only be implemented for primitive float types whose
/// genshin node-graph kernel IDs match those in the `#[native_calc(N)]`
/// attributes on each method. The current implementation targets `f32`
/// and uses kernels `221` (sqrt), `215` (log), `291-296` (trig). Implementing
/// this trait for any other type would dispatch to wrong kernel IDs and
/// produce a corrupt node graph.
pub unsafe trait F32 {
    fn sqrt(self) -> Self;
    fn log(self, base: Self) -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn tan(self) -> Self;
    fn asin(self) -> Self;
    fn acos(self) -> Self;
    fn atan(self) -> Self;
}
```

Commit:
```bash
cd F:/rust2genshin && git add lib/src/math.rs && git commit -m "docs(lib): add Safety sections to unsafe trait I32 and F32"
```

### Phase 5 — Hand-fix unused variables in WIP files

For each unused variable flagged by clippy, rename to prefix with `_`. Specifically:

**`core/src/compile/compile2.rs`** (~20 vars): `sess`, `cgcx`, `shared_emitter`, `prof`, `tm_factory`, `exported_symbols_for_lto`, `each_linked_rlib_for_lto`, `modules`, `config`, `metadata`, `outputs`, `module_name`, `methods`, `cgu_name`, `cost`, `tcx`, `crate_info`, `ongoing_codegen`, `incr_comp_session`, `compiled_modules`, `target_features`, `opt_level`, `thin`, `method`, `crate_info`, etc.

**`core/src/compile/optimize.rs`**: `from`, `to` (per `core/src/compile/optimize.rs:82:36`).

**Other non-WIP files** with unused vars:
- `core/src/compile/func.rs:1:5` — unused import `std::panic::catch_unwind`
- `core/src/compile/func.rs:2:5` — unused import `downcast::Downcast`
- `core/src/asset/value.rs:427:32` — unused `x`
- `core/src/parser.rs:113:34` — unused `tcx`

For unused vars: prefix with `_` (e.g., `sess` → `_sess`).
For unused imports: remove the `use` line entirely.

After each file's edit, re-run clippy to get the next round of specific warnings to address.

**Honoring the user's "keep TODOs on unimplemented features" preference:** the unused variables are placeholders for unimplemented compiler-backend features (codegen, optimization). We do NOT delete the variables — we prefix them with `_` so clippy is satisfied but the structure remains. Any `// TODO` comments tied to these variables stay in place.

Commit per file (or batched in a single commit per logical group):
```bash
cd F:/rust2genshin && git add core/src/compile/compile2.rs core/src/compile/optimize.rs && git commit -m "refactor(core): prefix unused vars with _ in WIP files"
# ... and separate commits for non-WIP cleanups
```

### Phase 6 — Hand-fix unused manifest dependencies

Remove five unused deps:
- Root `Cargo.toml`: `slotmap`, `indexmap`, `enum_dispatch` (workspace deps)
- `core/Cargo.toml`: `id-pool`
- `demo/Cargo.toml`: `rand`

For each, simply delete the corresponding line. Before committing, verify cargo still builds (the dep may be transitively needed even if not directly used).

Commit:
```bash
cd F:/rust2genshin && git add Cargo.toml core/Cargo.toml demo/Cargo.toml && git commit -m "chore: remove unused dependencies (id-pool, rand, slotmap, indexmap, enum_dispatch)"
```

### Phase 7 — Final verification

Re-run clippy:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep -E "^warning:" | wc -l
```

Expected: `0`.

If any warnings remain, address them in a follow-up commit (likely stragglers from edge cases — e.g., a newly-introduced clippy warning from one of the fixes).

Re-run the demo build pipeline and SHA-256 check:
```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
sha256sum F:/rust2genshin/target/rust2genshin_demo.gia
```

Expected SHA-256: `f3bc21afee2dc209664a760c8c79fd34b35292b098de44da09c79ac82d18e584` (unchanged from inline-polish baseline).

If different: revert the most recent commit and investigate.

Run tests:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

## Components

### Modified (potentially)
- `core/src/compile/mod.rs` — derivable_impls and/or needless_late_init fixes (Phase 1)
- `core/src/compile/func.rs` — unused imports + auto-fixes (Phases 1 & 5)
- `core/src/compile/native.rs` — diverging_sub_expression fix (Phase 2)
- `core/src/compile/compile2.rs` — unused variable `_` prefixing (Phase 5)
- `core/src/compile/optimize.rs` — unused variable `_` prefixing (Phase 5)
- `core/src/compile/parser.rs` — unused variable (Phase 5)
- `core/src/asset/value.rs` — derivable_impls + unused variable (Phases 1 & 5)
- `core/src/asset/mod.rs` — collapsible_if (Phase 1)
- `core/src/asset/node_graph/*.rs` — derivable_impls, needless_late_init, large_enum_variant (Phases 1 & 3)
- `core/src/asset/node_graph/arithmetic.rs` — needless_late_init (Phase 1)
- `lib/src/math.rs` — missing_safety_doc on `I32` and `F32` (Phase 4)
- `lib/src/dict.rs`, `lib/src/list.rs`, `lib/src/player.rs` — possible auto-fixes (Phase 1)
- `lib-internal/src/lib.rs` — possible auto-fixes (Phase 1)
- `demo/build.rs` — unused import (Phase 1)
- `demo/src/lib.rs` — possible auto-fixes (Phase 1)
- `Cargo.toml` — remove unused workspace deps (Phase 6)
- `core/Cargo.toml` — remove unused dep (Phase 6)
- `demo/Cargo.toml` — remove unused dep (Phase 6)

### Unchanged
- `.gia` output (verified by SHA-256 gate)
- Proto schema (`core/proto/asset.proto`)
- Behavior of all node generation, encoding, and serialization

## Data flow

N/A — this is purely a code-cleanup pass. No runtime behavior changes.

## Error handling

No changes to error paths. The judgment-call fixes (Phase 2, 3, 4) modify structure but preserve semantic behavior:
- Phase 2: removing an unreachable cast doesn't change runtime — `todo!()` panics regardless
- Phase 3: boxing `NodeInterfaceContainer` doesn't change protobuf encoding
- Phase 4: adding `# Safety` doc sections is comment-only

## Testing

After each phase:
```bash
cargo +nightly build -p rust2genshin   # must succeed
cargo +nightly test -p rust2genshin    # must pass 5/5
```

After Phase 7 (final):
```bash
cargo +nightly clippy --workspace --all-targets 2>&1 | grep -E "^warning:" | wc -l   # must be 0
cargo +nightly run -p build-demo
sha256sum target/rust2genshin_demo.gia                                              # must match baseline
```

## Risks

- **`cargo clippy --fix` may apply changes we don't want** — e.g., it might reformat code beyond the lint's suggestion. Mitigation: review each commit's diff carefully; revert any commit whose diff is broader than expected and re-apply by hand.
- **Phase 3 boxing may affect serialization** — `Box<T>` deref-serializes to the same bytes as `T` in prost, but this is not guaranteed for all field types. Mitigation: SHA-256 gate catches any divergence.
- **Phase 5 `_` prefixing in WIP files may be reverted by future feature work** — the `_` prefix is a clippy-only signal; when the feature lands and the var is used, the prefix will be removed naturally. No risk.
- **Auto-fix may produce duplicate commits** — phase 1's single commit could be large (~50 changes). Mitigation: commit one phase at a time, review the diff.

## Out-of-spec follow-ups

After this cleanup pass, the workspace has zero clippy warnings. Future code changes can use `cargo +nightly clippy --workspace --all-targets -- -D warnings` as a CI gate.

The remaining TODOs (in `core/src/compile/mod.rs`, `core/src/asset/node_graph/mod.rs:468-471`, etc.) and the WIP files (`compile2.rs`, `optimize.rs`) remain in place — those are tracked in the inline-polish spec's "Out of scope" section and will become their own sub-projects.