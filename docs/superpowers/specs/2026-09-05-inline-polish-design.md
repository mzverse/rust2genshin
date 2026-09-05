# Inline Polish — Design Spec

**Date:** 2026-09-05
**Status:** Approved (brainstorming complete)
**Scope:** Remove or document 7 inline `// TODO` markers that are stale, debug-leftovers, or undocumented-but-correct code. No behavior changes.

## Context

The `rust2genshin` codebase has accumulated a handful of `// TODO` comments across `core/src/compile/` and `lib/src/`. Many of them are **stale markers** rather than outstanding work — the code was written speculatively, and the features behind the TODOs were implemented later (cast, Entity, Guid) but the comments were never removed. A few are comment-only placeholders where the code is correct but undocumented.

This spec clears those markers without touching logic. The user's full remaining-todo list (`unsigned int`, `i64`, `tuple`, `struct`, `events`, `loops`, `Box`, `async`, `closure`, `client node graph`) is addressed by other sub-projects; this one is purely housekeeping.

## Scope

**In scope (Group A — 7 edits in 4 files):**

1. `core/src/compile/mod.rs:224` — clean up the span_err + expect + panic pattern.
2. `core/src/compile/mod.rs:265` — delete a commented-out debug `eprintln!`.
3. `core/src/compile/mod.rs:363` — document why `Side::Server` is hardcoded for locals.
4. `core/src/compile/func.rs:175` — remove a bare stale `// TODO` on `Operand::Copy`.
5. `lib/src/dict.rs:21` — remove a bare stale `// TODO`.
6. `lib/src/list.rs:4` — remove a `// TODO` on the placeholder `List` struct.
7. `core/src/asset/node_graph/mod.rs:468-471` — replace 4 `// TODO` markers with one explanatory comment block.

**Out of scope (deferred to separate sub-projects):**

- `lib/src/entity.rs:60-64` — three entity methods (IDs 245, 668, 250) plus a "modify model color/material" comment. These are API surface additions that need engine doc lookup; not polish.
- `lib/src/math.rs:5` — `Vec3` ops. API design question; needs user decisions on what to add.
- `lib/src/player.rs:4` — `// TODO: rename` on `get_player_id_by_guid`. Needs a name decision.
- `core/src/compile/mod.rs:305` — event_handler entrypoint management. Tied to the events Tier 2 feature.
- `core/src/compile/mod.rs:354` — adapt locals for struct/list/map. Tied to the struct Tier 2 feature.
- `core/src/asset/node_graph/composite.rs:97` — `// TODO: enum ...` on `encode_type_detail`. Tied to the enum support feature.
- `core/src/asset/node_graph/composite.rs:139` — `meta_pins: vec![] // TODO`. May already be correct (empty default); needs a separate investigation to confirm.

## Approach

Each edit is mechanical. Apply them in file order. After each edit, run `cargo +nightly build -p rust2genshin` to confirm the code still compiles (no semantic change should affect this; we are only adjusting diagnostics and comments).

### Edit 1 — `core/src/compile/mod.rs:224`

**Before** (lines around 224):

```rust
self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind())).expect("TODO: panic message");
panic!();
```

**After:**

```rust
let _ = self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind()));
panic!("Unsupported type: {:?}", ty.kind());
```

**Rationale:** The `expect("TODO: panic message")` and the second `panic!()` are both noise. The pattern is "emit diagnostic then panic"; a single `panic!` after the (ignored) diagnostic result is clearer. The `let _ =` suppresses the unused-`Result` warning; the `panic!` will not return.

### Edit 2 — `core/src/compile/mod.rs:265`

**Before** (line 265):

```rust
// eprintln!("{:?}", self.tcx.output_filenames(()).with_extension("gia")); // TODO
```

**After:** delete the entire line.

**Rationale:** Stale debug code, no longer needed (the actual output path is built a few lines later via `out_dir.join(...)`).

### Edit 3 — `core/src/compile/mod.rs:363`

**Before** (line 363):

```rust
if kind.encode_storage(Side::Server /*TODO*/).is_some() {
```

**After:**

```rust
if kind.encode_storage(Side::Server /* locals are server-side; SLocalVarRef has ClientUnknown */).is_some() {
```

**Rationale:** Hardcoding `Side::Server` here is correct — `node_local` (the value this `is_some` check gates) is a server-only concept; see `ValueLocalVarRef` which returns `ClientTypeId::ClientUnknown`. The TODO is just a stale marker; replacing it with an explanatory comment documents the choice.

### Edit 4 — `core/src/compile/func.rs:175`

**Before** (line 175):

```rust
Operand::Copy(p) | // TODO
Operand::Move(p) => {
```

**After:**

```rust
Operand::Copy(p) |
Operand::Move(p) => {
```

**Rationale:** `Operand::Copy` and `Operand::Move` are intentionally handled identically because `node_local` is a value-type holder — both arms just read the local's value-output pin. The TODO was a stale marker. The arm merging is correct.

### Edit 5 — `lib/src/dict.rs:21`

**Before** (line 21, last line of file):

```rust
// TODO
```

**After:** delete the line entirely. The file ends at line 20 (`unsafe impl DictKey for Entity {}`) with a trailing newline.

**Rationale:** The 4 `DictKey` impls (`i32`, `String`, `Guid`, `Entity`) are complete. The trailing bare `// TODO` is a stale marker.

### Edit 6 — `lib/src/list.rs:4`

**Before** (line 4):

```rust
pub struct List<T>(i32, PhantomData<T>); // TODO
```

**After:**

```rust
pub struct List<T>(i32, PhantomData<T>);
```

**Rationale:** `List` is an intentional placeholder for the eventual real list type. List-element methods (`push`, `get`, iteration) are deferred to a future sub-project (tied to list literal support). Removing the TODO doesn't claim the type is complete — it acknowledges the TODO is a placeholder marker rather than outstanding work. The `// TODO` would re-appear as a separate `// TODO: list methods` or be tracked in README if needed.

### Edit 7 — `core/src/asset/node_graph/mod.rs:468-471`

**Before** (lines 466-471):

```rust
x_pos: 0.,
y_pos: 0.,
attached_comment: None, // TODO
context_declaration: None, // TODO
signal_version: None, // TODO
using_structs: vec![], // TODO
```

**After** (replace the four `// TODO` markers and add a single block comment immediately before the struct literal):

```rust
// Backend-generated graphs: no UI comments, no context scope, no signal
// versioning, no struct usage. All four are intentionally empty defaults;
// populating them is deferred to the comments / events / struct sub-projects.
x_pos: 0.,
y_pos: 0.,
attached_comment: None,
context_declaration: None,
signal_version: None,
using_structs: vec![],
```

**Rationale:** Four separate `// TODO` markers carrying the same meaning are noise. One block comment is clearer and points to the sub-projects that would populate them.

## Components

### Modified

- `core/src/compile/mod.rs` — edits 1, 2, 3.
- `core/src/compile/func.rs` — edit 4.
- `lib/src/dict.rs` — edit 5.
- `lib/src/list.rs` — edit 6.
- `core/src/asset/node_graph/mod.rs` — edit 7.

### Unchanged

- All other files. No proto changes. No node definitions. No demo changes. No new tests.

## Data flow

N/A — these edits do not affect execution. The runtime behavior of `compile_ty`, `compile_fn`, and `NodeGraph::encode` is identical before and after.

## Error handling

No change to error paths. Edit 1 is the only edit that touches an error path, and it preserves behavior: the diagnostic is emitted first, then the panic still happens with the same message.

## Testing

There is no in-repo test harness for the backend. Verification is build-and-compare:

```shell
cargo +nightly build -p rust2genshin
cargo +nightly run -p build-demo
ls -la target/rust2genshin-demo.gia
sha256sum target/rust2genshin-demo.gia
```

Compare the `.gia` SHA-256 before and after the edits. Expected: **byte-identical output**. If they differ, an edit accidentally changed behavior — revert and investigate.

## Risks

- **Edit 7 location:** the comment block goes immediately before the struct literal at line 466. If the surrounding code is reorganized (unlikely in this scope), the comment may end up detached from the fields. Mitigation: place the comment right next to the four fields, not at file scope.
- **Edit 1 dead-code warning:** the `let _ = ...` binding is intentional to suppress unused-result. If the project's `#![deny(unused_must_use)]` is set in this file, the warning will fire. Mitigation: check; if so, use a `#[allow(unused_must_use)]` on the line or `let _ = (/* ... */).ok();` pattern.
- **Edit 4 verification:** `Operand::Copy` and `Operand::Move` semantics may differ for `Copy` vs non-`Copy` types. Since `node_local` always holds a value (not a reference), and the local's output pin is a value (always copyable), the merge is safe. If a future change introduces reference-holding locals, this assumption must be revisited.

## Out-of-spec follow-ups

After this polish pass, the remaining `// TODO` markers in the codebase are the **deferred** items listed in the Scope/Out-of-scope section above. They will each become their own brainstorm → spec → plan → implement cycle, following the same pattern as `cast` (Tier 1) and now `inline-polish` (Group A).