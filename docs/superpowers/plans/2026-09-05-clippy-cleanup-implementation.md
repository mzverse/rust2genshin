# Clippy Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all 127 clippy warnings across the workspace, ending with `cargo clippy --workspace --all-targets` showing zero warnings and the demo `.gia` output byte-identical to the inline-polish baseline (SHA-256 `f3bc21afee2dc209664a760c8c79fd34b35292b098de44da09c79ac82d18e584`).

**Architecture:** Seven sequential phases. Phase 1 runs `cargo clippy --fix` to apply ~50 machine-applicable fixes in one shot. Phases 2-6 hand-fix the judgment-call lints (diverging_sub_expression, large_enum_variant, missing_safety_doc, unused variables, unused manifest deps) one at a time. Phase 7 verifies clippy is clean and the `.gia` output is unchanged.

**Tech Stack:** Rust nightly, `cargo clippy --fix --allow-dirty --allow-no-vcs`, `cargo +nightly test -p rust2genshin`, `cargo +nightly run -p build-demo`, `sha256sum` for output comparison.

**Project root:** `F:/rust2genshin/`

---

## File Structure

**Modified (potentially, by phase):**
- Phase 1: any file with auto-fixable lints (most of `core/src/asset/`, parts of `core/src/compile/`, `lib/src/`, `demo/`)
- Phase 2: `core/src/compile/native.rs`
- Phase 3: `core/src/asset/node_graph/*.rs` (file TBD — find via clippy output)
- Phase 4: `lib/src/math.rs`
- Phase 5: `core/src/compile/compile2.rs`, `core/src/compile/optimize.rs`, `core/src/compile/func.rs`, `core/src/asset/value.rs`, `core/src/parser.rs`, plus any others clippy flags
- Phase 6: root `Cargo.toml`, `core/Cargo.toml`, `demo/Cargo.toml`
- Phase 7: no file changes

**Unchanged:** the protobuf output, `core/proto/asset.proto`, behavior of node generation.

---

## Task 1: Phase 1 — Run `cargo clippy --fix` for auto-fixable lints

**Files:**
- Modified: many files across the workspace, determined by clippy

- [ ] **Step 1: Run clippy --fix**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets --fix --allow-dirty --allow-no-vcs
```

Expected: clippy prints a summary of fixes applied. No build errors.

- [ ] **Step 2: Verify the build still passes**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds (warnings allowed, but no errors).

- [ ] **Step 3: Verify the tests still pass**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 tests pass (`dict_encodes_pairs`, `list_types_encode`, `scalar_types_encode_storage`, `server_only_types_use_client_unknown`, `struct_value_encodes`).

- [ ] **Step 4: Inspect the diff for unintended changes**

Run:
```bash
cd F:/rust2genshin && git diff --stat
```

Expected: many files modified, but each diff is small and targeted (add `#[derive(Default)]`, drop `.clone()`, drop `Some(x).ok()`, etc.).

If any file's diff is broader than expected (e.g., reformatting the whole file, changing logic), revert just that file with `git checkout HEAD -- <path>` and hand-fix the lint instead. Then proceed.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add -A && git commit -m "refactor: apply cargo clippy --fix for auto-fixable warnings"
```

---

## Task 2: Phase 2 — Hand-fix `diverging_sub_expression` at `native.rs:22`

**Files:**
- Modify: `core/src/compile/native.rs:22`

- [ ] **Step 1: Locate the line**

Open `F:/rust2genshin/core/src/compile/native.rs`. Around line 22, inside the intrinsic-match block, find:

```rust
"black_box" => todo!(),
other => todo!("intrinsic: {other}") as Result<_>,
```

Context: `todo!()` diverges (returns `!`), so the `as Result<_>` cast is unreachable — clippy's `diverging_sub_expression` lint flags this.

- [ ] **Step 2: Replace the second arm**

Replace:
```rust
other => todo!("intrinsic: {other}") as Result<_>,
```

with:
```rust
other => todo!("intrinsic: {other}"),
```

Do not change anything else.

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

If the match expression's inferred return type breaks (because `todo!()` arms return `!`), the compiler will report a type-mismatch. If so, wrap the match in an explicit annotation:

```rust
let result: Result<NodeKind> = match self.tcx.intrinsic(def_id).unwrap().name.as_str() {
    "black_box" => todo!(),
    other => todo!("intrinsic: {other}"),
};
result.into()
```

But in practice Rust's divergence propagation should handle it without changes.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/native.rs && git commit -m "fix(core): drop unreachable 'as Result<_>' cast in intrinsic match"
```

---

## Task 3: Phase 3 — Hand-fix `large_enum_variant` (box the variant)

**Files:**
- Modify: the file flagged by clippy (TBD — to be located in Step 1)

- [ ] **Step 1: Locate the exact site**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep -B 2 -A 8 "large_enum_variant"
```

Expected output: a clippy warning showing the enum variant line and the boxing suggestion. The variant will be `InterfaceData(super::NodeInterfaceContainer)` based on the previous clippy run, but verify the current location.

- [ ] **Step 2: Apply the boxing change**

In the file identified by Step 1, find the enum variant:
```rust
InterfaceData(super::NodeInterfaceContainer),
```

Replace with:
```rust
InterfaceData(Box<super::NodeInterfaceContainer>),
```

- [ ] **Step 3: Update match arms and constructors**

Search for all uses of `InterfaceData(...)` in the workspace:
```bash
cd F:/rust2genshin && grep -rn "InterfaceData(" --include="*.rs"
```

For each `InterfaceData(value)` where `value` is a `NodeInterfaceContainer`, wrap with `Box::new(...)`:
```rust
InterfaceData(Box::new(value))
```

For destructuring patterns (`InterfaceData(inner)` → `inner`), the deref-coercion should handle the access without changes (Box<T> implements Deref<Target=T>).

- [ ] **Step 4: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

If it fails with type-mismatch in match patterns or constructors, add `.as_ref()` or `*` deref as needed.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add core/src/asset/node_graph/ && git commit -m "refactor(core): box large InterfaceData variant per clippy suggestion"
```

---

## Task 4: Phase 4 — Add `# Safety` sections to `unsafe trait I32` and `F32`

**Files:**
- Modify: `lib/src/math.rs:19` (I32 trait)
- Modify: `lib/src/math.rs:24` (F32 trait)

- [ ] **Step 1: Open the file and locate both traits**

Open `F:/rust2genshin/lib/src/math.rs`. Find:
- Line 19: `pub unsafe trait I32 {`
- Line 24: `pub unsafe trait F32 {`

- [ ] **Step 2: Add `# Safety` to `I32`**

Replace:
```rust
pub unsafe trait I32 {
    fn ushr(self, rhs: Self) -> Self;
    fn shr(self, rhs: Self) -> Self;
}
```

with:
```rust
/// Integer math helpers backed by genshin node-graph kernel operations.
///
/// # Safety
///
/// This trait must only be implemented for primitive integer types whose
/// genshin node-graph kernel IDs match those in the `#[native_calc(N)]`
/// attributes on each method. The current implementation targets `i32` and
/// uses kernel `779` (ushr) plus an inlined shr. Implementing this trait
/// for any other type would dispatch to wrong kernel IDs and produce a
/// corrupt node graph.
pub unsafe trait I32 {
    fn ushr(self, rhs: Self) -> Self;
    fn shr(self, rhs: Self) -> Self;
}
```

- [ ] **Step 3: Add `# Safety` to `F32`**

Replace:
```rust
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

with:
```rust
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

- [ ] **Step 4: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds (the doc comments don't affect compilation).

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add lib/src/math.rs && git commit -m "docs(lib): add Safety sections to unsafe trait I32 and F32"
```

---

## Task 5: Phase 5 — Hand-fix unused variables and imports

**Files:**
- Modify: `core/src/compile/compile2.rs` — ~20 unused vars, prefix with `_`
- Modify: `core/src/compile/optimize.rs` — `from`, `to` → `_from`, `_to`
- Modify: `core/src/compile/func.rs` — remove unused imports `std::panic::catch_unwind`, `downcast::Downcast`
- Modify: `core/src/asset/value.rs` — `x` → `_x` (line 427)
- Modify: `core/src/parser.rs` — `tcx` → `_tcx` (line 113)
- Modify: any other file clippy flags in a fresh run after Phase 1

- [ ] **Step 1: Get the current list of unused-variable warnings**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep -B 1 "unused variable" | grep -E "(-->|\|)"
```

Expected: a list of `file:line:column` references for each unused variable.

- [ ] **Step 2: Prefix unused variables in `core/src/compile/compile2.rs` with `_`**

Open `F:/rust2genshin/core/src/compile/compile2.rs`. For each unused variable flagged in Step 1 within this file, rename it to prefix with `_`.

For example, if `sess` is flagged at line 33, change every occurrence of `sess` in the function to `_sess`. This typically means renaming the parameter binding AND any uses within the function.

If a function has multiple unused parameters, prefix each:
```rust
fn codegen(_sess: &Session, _cgcx: &CodegenCx<Self>, ...) { ... }
```

- [ ] **Step 3: Prefix unused variables in `core/src/compile/optimize.rs` with `_`**

Open `F:/rust2genshin/core/src/compile/optimize.rs`. Apply the same prefix-with-`_` treatment for any unused variables.

- [ ] **Step 4: Remove unused imports in `core/src/compile/func.rs`**

Open `F:/rust2genshin/core/src/compile/func.rs`. The unused imports are:
```rust
use std::panic::catch_unwind;
use downcast::Downcast;
```

Delete those two lines entirely. Do not change any other line.

- [ ] **Step 5: Prefix other unused variables across the workspace**

For each remaining file with unused-variable warnings (e.g., `core/src/asset/value.rs`, `core/src/parser.rs`), open the file and prefix the unused variable with `_`.

- [ ] **Step 6: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds with no new warnings on the modified lines.

- [ ] **Step 7: Test to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 tests pass.

- [ ] **Step 8: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/compile2.rs core/src/compile/optimize.rs core/src/compile/func.rs core/src/asset/value.rs core/src/parser.rs && git commit -m "refactor: prefix unused vars with _ and remove unused imports"
```

---

## Task 6: Phase 6 — Remove unused manifest dependencies

**Files:**
- Modify: root `Cargo.toml` (workspace deps)
- Modify: `core/Cargo.toml`
- Modify: `demo/Cargo.toml`

- [ ] **Step 1: Verify each dep is truly unused**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep -B 1 "unused dependency"
```

Expected: 5 entries — `id-pool`, `rand`, `slotmap`, `indexmap`, `enum_dispatch`.

- [ ] **Step 2: Remove `slotmap`, `indexmap`, `enum_dispatch` from root `Cargo.toml`**

Open `F:/rust2genshin/Cargo.toml`. In the `[workspace.dependencies]` section, delete the three lines for `slotmap`, `indexmap`, `enum_dispatch`.

- [ ] **Step 3: Remove `id-pool` from `core/Cargo.toml`**

Open `F:/rust2genshin/core/Cargo.toml`. Find `[dependencies]` section and delete the line `id-pool = "0"`.

- [ ] **Step 4: Remove `rand` from `demo/Cargo.toml`**

Open `F:/rust2genshin/demo/Cargo.toml`. Find `[dependencies]` section and delete the line `rand = "0.10.2"`.

- [ ] **Step 5: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds. If any dep is actually used transitively or directly, the build will fail with a missing-crate error — in that case, revert that specific deletion and report.

- [ ] **Step 6: Test to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 tests pass.

- [ ] **Step 7: Commit**

```bash
cd F:/rust2genshin && git add Cargo.toml core/Cargo.toml demo/Cargo.toml && git commit -m "chore: remove unused dependencies (id-pool, rand, slotmap, indexmap, enum_dispatch)"
```

---

## Task 7: Phase 7 — Final verification

**Files:**
- No file changes. This task only verifies the work.

- [ ] **Step 1: Confirm clippy is clean**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep -E "^warning:" | wc -l
```

Expected: `0`.

If non-zero: list the remaining warnings with `cargo +nightly clippy --workspace --all-targets 2>&1 | grep -B 1 -A 3 "^warning:"`. Address them by hand-fix in a follow-up commit (this task is supposed to leave clippy clean — if there are stragglers, fix them and amend this task's commit if not yet pushed, or create a new commit).

- [ ] **Step 2: Re-run the demo pipeline**

Run:
```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: pipeline completes; `target/rust2genshin_demo.gia` exists.

- [ ] **Step 3: Confirm SHA-256 matches baseline**

Run:
```bash
cd F:/rust2genshin && sha256sum target/rust2genshin_demo.gia
```

Expected output: `f3bc21afee2dc209664a760c8c79fd34b35292b098de44da09c79ac82d18e584  target/rust2genshin_demo.gia`.

If different: revert the most recent clippy-related commit and re-verify. The first commit whose revert restores the baseline SHA identifies the offending change.

- [ ] **Step 4: Final test run**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

The clippy-cleanup sub-project is complete when all four steps pass. Seven commits (one per phase) plus any follow-ups are on the branch.