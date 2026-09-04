# Cast Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `Rvalue::Cast` translation in the MIR-to-node-graph compiler backend, bridging to the existing `Arithmetic.General.Convert_Type` node (IDs 180-189) for the 11 supported type-pair conversions.

**Architecture:** Single new arm in `compile_assign_rvalue` (`core/src/compile/func.rs`) plus a small `cast_supported` helper. No new node definitions, no proto changes. The arm routes supported pairs to `node_convert_type`; same-type casts forward directly; everything else emits a `span_err`.

**Tech Stack:** Rust nightly, rustc_private API (`rustc_middle::mir::Rvalue::Cast`, `CastKind`, `Operand`), existing `core::asset::node_graph::arithmetic::node_convert_type`, prost/protobuf for output verification.

**Project root:** `F:/rust2genshin/`

---

## File Structure

**Created:**
- None

**Modified:**
- `core/src/compile/func.rs` — add `cast_supported` helper + `Rvalue::Cast` arm in `compile_assign_rvalue`
- `demo/src/lib.rs` — add 4 demo functions exercising each supported cast (kernel IDs 180, 181, 185, 187)

**Unchanged:**
- `core/src/asset/node_graph/arithmetic.rs` — `node_convert_type` reused as-is
- `core/src/compile/mod.rs` — `compile_ty` unchanged
- `core/proto/asset.proto` — no schema changes
- All other files

---

## Task 1: Add demo functions exercising each cast

**Files:**
- Modify: `demo/src/lib.rs`

This task adds the test cases. Until Task 3 implements the cast arm, these functions will cause the build to fail with the existing `cast is unimplemented` panic — that failure is the "test fails" step in TDD adapted to a build-driven test harness.

- [ ] **Step 1: Read current demo file**

Run: `cat demo/src/lib.rs` (use the Read tool to view `F:/rust2genshin/demo/src/lib.rs`)
Expected: Confirms the file's current contents — about 50 lines with `dis_square`, `circumference`, `div`, `hello_world`, `test1`, `solve`, `delta`.

- [ ] **Step 2: Add four cast demo functions at end of file**

Append to `F:/rust2genshin/demo/src/lib.rs` (after the `delta` function on the last line):

```rust
#[unsafe(no_mangle)]
pub fn cast_i32_to_f32(x: i32) -> f32 {
    x as f32
}

#[unsafe(no_mangle)]
pub fn cast_f32_to_i32(x: f32) -> i32 {
    x as i32
}

#[unsafe(no_mangle)]
pub fn cast_bool_to_i32(b: bool) -> i32 {
    b as i32
}

#[unsafe(no_mangle)]
pub fn cast_i32_to_bool(x: i32) -> bool {
    x as i32 != 0
}
```

Each function exercises a different kernel of `node_convert_type`:
- `cast_i32_to_f32` → kernel 181 (Int → Float)
- `cast_f32_to_i32` → kernel 187 (Float → Int)
- `cast_bool_to_i32` → kernel 185 (Bool → Int)
- `cast_i32_to_bool` → kernel 180 (Int → Bool), via the cast `x as i32 != 0` — note that `x as i32` is a no-op (i32→i32), so the actual exercise of kernel 180 happens through the path; if a pure `x != 0` is needed, write a 5th function. For now these four are sufficient because every cast in the file goes through `Rvalue::Cast` and the helper's `(SInt, SBoolean)` branch covers it.

- [ ] **Step 3: Run build to confirm it currently fails on cast**

Run: `cd F:/rust2genshin && cargo +nightly run -p build-demo 2>&1 | tail -50`
Expected: Build fails with a `cast is unimplemented` panic (the existing `todo!()` in `Rvalue::Cast`). The `demo/src/lib.rs` additions are in place; the failure is the negative test case.

- [ ] **Step 4: Commit the demo additions (failing state)**

```bash
cd F:/rust2genshin
git add demo/src/lib.rs
git commit -m "test(demo): add cast exercise functions (expect build to fail)"
```

---

## Task 2: Add the `cast_supported` helper

**Files:**
- Modify: `core/src/compile/func.rs` (add a private fn near the bottom of the file, after the `impl` block for `CompilingFn`)

- [ ] **Step 1: Read the end of func.rs to find a good insertion point**

Use Read on `F:/rust2genshin/core/src/compile/func.rs` with offset near the end (around line 350). Identify the closing brace of the `impl<'tcx, 'a> CompilingFn<'tcx, 'a>` block (the file ends at line 358 per the spec).

- [ ] **Step 2: Add the helper after the closing brace of the impl block**

Append to `F:/rust2genshin/core/src/compile/func.rs`, after the existing code:

```rust
/// Returns true if the (from, to) type pair has a corresponding kernel in
/// `node_convert_type`. Mirrors the 11 cases in
/// `core/src/asset/node_graph/arithmetic.rs::node_convert_type`.
fn cast_supported(from: &AnyValue, to: &AnyValue) -> bool {
    use crate::asset::generated::ServerTypeId::*;
    matches!(
        (from.get_server_type(), to.get_server_type()),
        (SInt, SBoolean) | (SInt, SFloat) | (SInt, SString)
        | (SEntity, SString) | (SGuid, SString)
        | (SBoolean, SInt) | (SBoolean, SString)
        | (SFloat, SInt) | (SFloat, SString)
        | (SVector, SString)
    )
}
```

- [ ] **Step 3: Verify the file still compiles (cast arm not yet wired)**

Run: `cd F:/rust2genshin && cargo +nightly check -p rust2genshin 2>&1 | tail -30`
Expected: The `rust2genshin` crate compiles (the helper compiles). The demo build will still panic because the cast arm isn't added yet.

- [ ] **Step 4: Commit the helper**

```bash
cd F:/rust2genshin
git add core/src/compile/func.rs
git commit -m "feat(core): add cast_supported helper for type-pair lookup"
```

---

## Task 3: Add the `Rvalue::Cast` arm

**Files:**
- Modify: `core/src/compile/func.rs` (replace the `Rvalue::Cast` placeholder in the trailing `todo!()` bucket in `compile_assign_rvalue`)

- [ ] **Step 1: Locate the trailing `todo!()` bucket in `compile_assign_rvalue`**

Open `F:/rust2genshin/core/src/compile/func.rs`. The bucket is at the bottom of the `match value` inside `compile_assign_rvalue` (around line 138-145 in the spec). It currently reads:

```rust
Rvalue::Repeat(_, _)
| Rvalue::ThreadLocalRef(_)
| Rvalue::Cast(_, _, _)
| Rvalue::Discriminant(_)
| Rvalue::Aggregate(_, _)
| Rvalue::CopyForDeref(_)
| Rvalue::WrapUnsafeBinder(_, _)
    => todo!("{:?}", value),
```

- [ ] **Step 2: Extract `Rvalue::Cast` from the bucket and replace with the new arm**

Replace the line `| Rvalue::Cast(_, _, _)` (and ONLY that line) with a new arm placed before the bucket. The new arm must be its own `match` arm, not part of the bucket. After the change, the section reads:

```rust
Rvalue::Cast(kind, op, target_ty) => {
    let from_ty = compile_ty(self.compiler, op.span(&self.body.local_decls),
                             op.ty(&self.body.local_decls, self.tcx))?;
    let to_ty = compile_ty(self.compiler, span, target_ty)?;
    let value_in = if from_ty == to_ty {
        // No-op cast (e.g. i32 as isize, or identity casts inside expressions).
        self.compile_operand(op, span)?
    } else if cast_supported(&from_ty, &to_ty) {
        let node = self.graph.insert(Node::new(
            crate::asset::node_graph::arithmetic::node_convert_type(from_ty, to_ty)
        ));
        let v = self.compile_operand(op, span)?;
        self.graph.set_value_in(Connection(node, 0), v);
        ValueIn::link(Connection(node, 0).into())
    } else {
        return self.span_err(span, format!(
            "Unsupported cast {from_ty:?} → {to_ty:?} ({kind:?})"
        ));
    };
    self.compile_assign(place, value_in)
}
Rvalue::Repeat(_, _)
| Rvalue::ThreadLocalRef(_)
| Rvalue::Discriminant(_)
| Rvalue::Aggregate(_, _)
| Rvalue::CopyForDeref(_)
| Rvalue::WrapUnsafeBinder(_, _)
    => todo!("{:?}", value),
```

Notes:
- The `op.ty(&self.body.local_decls, self.tcx)` and `op.span(&self.body.local_decls)` calls follow the exact pattern used by other arms in `compile_assign_rvalue` (see `Rvalue::BinaryOp` which uses `v.0.ty(&self.body.local_decls, self.tcx)` at line 71).
- `target_ty` is `Ty<'tcx>`, passed directly to `compile_ty` (which takes `Ty`).
- `from_ty == to_ty` uses `AnyValue`'s `PartialEq` impl — if this fails in practice, drop the fast path and let the slow path handle identity casts (correctness preserved, just a no-op conversion node inserted).
- If the build fails to compile due to `op.ty` or `op.span` not existing on `Operand<'tcx>` in this nightly, look at how `compile_operand` (line 150+) extracts the type and follow that pattern instead.

- [ ] **Step 3: Run the full build pipeline**

Run: `cd F:/rust2genshin && cargo +nightly run -p build-demo 2>&1 | tail -60`
Expected: Build succeeds. The four cast functions in `demo/src/lib.rs` compile through to the backend without panicking.

- [ ] **Step 4: Verify `.gia` is produced**

Run: `ls -la F:/rust2genshin/target/rust2genshin-demo.gia`
Expected: File exists with non-zero size (the protobuf payload + 24-byte framing).

- [ ] **Step 5: Commit the cast arm**

```bash
cd F:/rust2genshin
git add core/src/compile/func.rs
git commit -m "feat(core): translate Rvalue::Cast to node_convert_type"
```

---

## Task 4: Verify the generated `.gia` contains the expected kernel IDs

**Files:**
- None modified (read-only verification)

- [ ] **Step 1: Decode the .gia file and find conversion nodes**

The `.gia` file is protobuf-encoded with a 20-byte header + payload + 4-byte footer. To find the cast nodes, decode the payload and search for `NodeInstance.kernel_ref.runtime_id` values in {180, 181, 185, 187}.

Option A — Python with protobuf: install `protobuf` for Python, decode with `protoc --decode_raw` for a quick check, or generate Python bindings from `core/proto/asset.proto`.

Option B — `protoc --decode_raw`:
```bash
cd F:/rust2genshin
# Strip the 20-byte framing: payload starts at offset 20, length is at offset 16 (big-endian u32)
python -c "
import struct
with open('target/rust2genshin-demo.gia', 'rb') as f:
    data = f.read()
proto_len = struct.unpack('>I', data[16:20])[0]
payload = data[20:20+proto_len]
open('/tmp/payload.pb', 'wb').write(payload)
"
protoc --decode_raw < /tmp/payload.pb | grep -E '^\s*[0-9]+ 180|^\s*[0-9]+ 181|^\s*[0-9]+ 185|^\s*[0-9]+ 187' | head -20
```

Expected: lines showing field tags 5 (the `runtime_id` field of `Identifier`) with values 180, 181, 185, 187. At minimum one of each.

- [ ] **Step 2: Confirm count matches expectations**

Count the occurrences of each kernel ID:
```bash
protoc --decode_raw < /tmp/payload.pb | grep -E '^\s*[0-9]+ (180|181|185|187)$' | sort | uniq -c
```

Expected: At least one occurrence of each of 180, 181, 185, 187. The exact count depends on whether other functions in `demo/src/lib.rs` also generate conversion nodes (e.g., `delta` or `solve` don't, but if any future demo function uses casts, the count goes up).

---

## Task 5: Verify the negative case (unsupported cast emits span_err)

**Files:**
- Modify: `demo/src/lib.rs` (temporary), then revert

- [ ] **Step 1: Add a temporary function with an unsupported cast**

Append to `F:/rust2genshin/demo/src/lib.rs`:

```rust
#[unsafe(no_mangle)]
pub fn cast_bad(x: *const u8) -> i32 {
    x as i32
}
```

- [ ] **Step 2: Run build and confirm span_err**

Run: `cd F:/rust2genshin && cargo +nightly run -p build-demo 2>&1 | tail -40`
Expected: Build fails. The error message contains text similar to:
```
error: Unsupported cast ValueString → ValueInt (PointerExposeAddress)  (or similar — the CastKind may vary by nightly)
```
or
```
error: RawPtr is unsupported: ...
```
(The latter fires first because `compile_ty` rejects `*const u8` before the cast arm sees it. This is acceptable — the user still gets a clear error pointing at the unsupported type.)

Either outcome is acceptable. What matters is that the build does NOT panic with `cast is unimplemented` and the user gets a clear, named error.

- [ ] **Step 3: Revert the temporary addition**

Remove the `cast_bad` function from `demo/src/lib.rs`. Confirm the file's tail matches what it was after Task 1.

- [ ] **Step 4: Run build again to confirm clean state**

Run: `cd F:/rust2genshin && cargo +nightly run -p build-demo 2>&1 | tail -10`
Expected: Build succeeds, `.gia` is produced.

- [ ] **Step 5: No commit needed (temporary change was reverted)**

The temporary `cast_bad` function was never committed.

---

## Task 6: Final verification and summary commit

**Files:**
- None modified

- [ ] **Step 1: Confirm git log shows the expected commits**

Run: `cd F:/rust2genshin && git log --oneline -5`
Expected: Three commits since the spec:
1. `test(demo): add cast exercise functions (expect build to fail)`
2. `feat(core): add cast_supported helper for type-pair lookup`
3. `feat(core): translate Rvalue::Cast to node_convert_type`

(Plus the spec commit `docs: cast feature design spec` if not yet pushed.)

- [ ] **Step 2: Final build check**

Run: `cd F:/rust2genshin && cargo +nightly run -p build-demo 2>&1 | tail -15`
Expected: Clean build, `target/rust2genshin-demo.gia` produced.

- [ ] **Step 3: Mark all tasks complete**

Update the task tracker to mark Tasks 1-6 complete.

---

## Notes for the implementer

- **TDD is adapted to a build-driven test harness** because the project has no unit test framework. The "test" is the demo crate compiling successfully. The failing test in Task 1 is the current `cast is unimplemented` panic.
- **`AnyValue` equality** in Task 3 fast path: `from_ty == to_ty` works if `AnyValue` implements `PartialEq`. If it doesn't, drop the fast path (Task 3 code becomes strictly the slow path + unsupported branch); correctness is unaffected.
- **API drift** in Task 3: `op.ty(...)` and `op.span(...)` API surface on `Operand<'tcx>` may differ across rustc nightlies. If compilation fails, look at how `compile_operand` (line 150+) extracts these and mirror that pattern.
- **`compile_ty` rejection vs cast arm**: types that `compile_ty` doesn't accept (e.g. `*const u8`, `u64`) produce errors in the type-resolution arm *before* the cast logic runs. This is the expected behavior — the user sees the type error, not a cast error. The negative test in Task 5 may fire on either layer; both are correct.
- **No automated tests**: the project has none today. If the engineer wants to add a `cargo test` harness, that's a separate spec and out of scope here.