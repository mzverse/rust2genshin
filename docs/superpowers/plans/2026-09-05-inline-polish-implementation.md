# Inline Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply 3 mechanical edits to `core/src/compile/mod.rs` and `core/src/compile/func.rs` to clean up stale `// TODO` markers and the diagnostic-noise pattern around the `Unsupported type` panic. Verify the `.gia` output is byte-identical to the pre-edit baseline.

**Architecture:** No new logic. Each edit adjusts a comment or diagnostic expression. Verification is build-and-compare: capture the `target/rust2genshin-demo.gia` SHA-256 before any edits, apply all three edits, then re-build and confirm the SHA-256 is unchanged. If the SHA changes, an edit accidentally affected behavior — revert and investigate.

**Tech Stack:** Rust nightly (rustc_private), existing `core/src/compile/{mod,func}.rs`, `cargo +nightly build -p rust2genshin` and `cargo +nightly run -p build-demo` for verification, `sha256sum` for output comparison.

**Project root:** `F:/rust2genshin/`

---

## File Structure

**Modified:**
- `core/src/compile/mod.rs` — edits 1 and 2 (lines 224 and 363).
- `core/src/compile/func.rs` — edit 3 (line 175).

**Unchanged:**
- All other files. No proto changes. No node definitions. No demo changes. No new tests.

**No new files.**

---

## Task 1: Capture baseline `.gia` SHA-256

**Files:**
- No file changes. This task only records a value.

- [ ] **Step 1: Build the backend**

Run:
```shell
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds with no errors (warnings allowed).

- [ ] **Step 2: Run the demo build pipeline to produce `.gia`**

Run:
```shell
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: pipeline completes; `target/rust2genshin-demo.gia` exists.

- [ ] **Step 3: Record the SHA-256**

Run:
```shell
sha256sum F:/rust2genshin/target/rust2genshin-demo.gia
```

Expected output: a SHA-256 hex digest on stdout, e.g. `abc123... target/rust2genshin-demo.gia`.

Copy that hex digest and store it locally (e.g. in a scratch file `F:/rust2genshin/.remember/tmp/inline-polish-baseline.sha`) — you'll compare it against the post-edit SHA in Task 5.

- [ ] **Step 4: Commit a no-op marker so the diff is clean**

There is nothing to commit yet; this task only captures a baseline. Skip the commit step. Proceed to Task 2.

---

## Task 2: Edit 1 — clean up `mod.rs:224` panic pattern

**Files:**
- Modify: `core/src/compile/mod.rs:223-225`

- [ ] **Step 1: Open the file and locate the lines**

Open `F:/rust2genshin/core/src/compile/mod.rs`. Around line 224, in the trailing arm of the `compile_ty` match, find these three lines:

```rust
self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind())).expect("TODO: panic message");
panic!();
```

Context: this is the catch-all arm of the `match ty.kind()` block. The `span_err` returns `Err` (see `WithTcx::span_err` in this same file). Calling `.expect("TODO: panic message")` on that `Err` immediately panics with the placeholder message — so the second `panic!()` on the next line is currently **unreachable**.

- [ ] **Step 2: Replace the three lines with two lines**

Replace those three lines with:

```rust
let _ = self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind()));
panic!("Unsupported type: {:?}", ty.kind());
```

The result section of `compile_ty` is `Ok(...)`, so the surrounding match arm must still produce an `AnyValue`. The two lines above intentionally panic after the diagnostic, so they never fall through. Do not add a return value.

- [ ] **Step 3: Build to confirm it compiles**

Run:
```shell
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

If the compiler warns `unused_must_use` for the `let _ = ...` (because `span_err` returns `Result`), use this alternative instead:

```rust
let _ = self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind())).ok();
panic!("Unsupported type: {:?}", ty.kind());
```

The `.ok()` discards the `Err` and yields `Option<()>`, which is fine to ignore via `let _ =`.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "refactor(core): clean up unsupported-type panic pattern in compile_ty"
```

---

## Task 3: Edit 2 — document `Side::Server` rationale at `mod.rs:363`

**Files:**
- Modify: `core/src/compile/mod.rs:363`

- [ ] **Step 1: Locate the line**

Open `F:/rust2genshin/core/src/compile/mod.rs`. Around line 363, inside `compile_fn`, find this line:

```rust
if kind.encode_storage(Side::Server /*TODO*/).is_some() {
```

Context: this is the per-local default-initializer guard inside `compile_fn`. The hardcoded `Side::Server` is correct because `node_local` is server-side (`ValueLocalVarRef` returns `ClientTypeId::ClientUnknown`). The `/*TODO*/` is a stale marker.

- [ ] **Step 2: Replace the inline comment**

Replace the line with:

```rust
if kind.encode_storage(Side::Server /* locals are server-side; SLocalVarRef has ClientUnknown */).is_some() {
```

Do not change anything else on the line.

- [ ] **Step 3: Build to confirm it compiles**

Run:
```shell
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "docs(core): explain why Side::Server is hardcoded for locals"
```

---

## Task 4: Edit 3 — remove stale `// TODO` after `Operand::Copy(p)` at `func.rs:175`

**Files:**
- Modify: `core/src/compile/func.rs:175`

- [ ] **Step 1: Locate the line**

Open `F:/rust2genshin/core/src/compile/func.rs`. Around line 175, inside `compile_operand`, find:

```rust
Operand::Copy(p) | // TODO
Operand::Move(p) => {
```

Context: `Operand::Copy` and `Operand::Move` are intentionally handled identically because `node_local` is a value-type holder — both arms just read the local's value-output pin via `Connection(*self.locals.get(p.local).unwrap(), 1)`. The TODO was a stale marker.

- [ ] **Step 2: Remove the trailing `// TODO`**

Replace:

```rust
Operand::Copy(p) | // TODO
Operand::Move(p) => {
```

with:

```rust
Operand::Copy(p) |
Operand::Move(p) => {
```

Do not change anything else.

- [ ] **Step 3: Build to confirm it compiles**

Run:
```shell
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "refactor(core): remove stale TODO on Operand::Copy arm in compile_operand"
```

---

## Task 5: Verify `.gia` SHA-256 is byte-identical to baseline

**Files:**
- No file changes. This task only verifies behavior.

- [ ] **Step 1: Re-run the demo pipeline**

Run:
```shell
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: pipeline completes; `target/rust2genshin-demo.gia` exists.

- [ ] **Step 2: Compute the new SHA-256**

Run:
```shell
sha256sum F:/rust2genshin/target/rust2genshin-demo.gia
```

Expected output: a SHA-256 hex digest on stdout.

- [ ] **Step 3: Compare to the baseline from Task 1**

Compare the new SHA-256 hex digest to the value stored in `F:/rust2genshin/.remember/tmp/inline-polish-baseline.sha`.

Expected: **identical**. The three edits are all comment/diagnostic-text changes; they do not affect any code path that contributes to the `.gia` output.

- [ ] **Step 4: If the SHA matches, clean up the scratch file and we're done**

If identical:
```bash
rm F:/rust2genshin/.remember/tmp/inline-polish-baseline.sha
```

Stop — the inline-polish sub-project is complete. Three commits are on the branch.

- [ ] **Step 5: If the SHA differs, revert and investigate**

If different, one of the edits accidentally changed behavior. Investigate by reverting one edit at a time (use `git revert <commit>` for each of the three commits, build, compare SHA). The first revert that restores the baseline SHA identifies the offending edit; inspect it and either fix or re-spec.

Do NOT proceed past this step without the SHAs matching.