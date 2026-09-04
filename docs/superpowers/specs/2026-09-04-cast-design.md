# Cast — Design Spec

**Date:** 2026-09-04
**Status:** Approved (brainstorming complete)
**Scope:** Implement `Rvalue::Cast` translation in the MIR→node-graph backend.

## Context

The `rust2genshin` codegen backend (`core/`) translates Rust MIR into Genshin Impact node-graph assets. `compile_assign_rvalue` in `core/src/compile/func.rs` currently has a `todo!()` arm for `Rvalue::Cast`, so any `as` expression in user code panics the compiler with `cast is unimplemented` (effectively).

Genshin's node-graph already has a **type-conversion node** (`Arithmetic.General.Convert_Type`, ID 180) with 11 type-pair kernel variants (IDs 180-189). See `core/src/asset/node_graph/arithmetic.rs` lines 603-630. The node is wired up but never invoked from `compile_assign_rvalue`.

This spec adds the bridge: `Rvalue::Cast` → `node_convert_type` for the type pairs the backend already supports.

## Scope

**In scope:**

- Casts between types `compile_ty` already accepts: `bool`, `i32`, `isize`, `f32`, `String`.
- The 11 type-pair combinations that `node_convert_type` supports.
- No-op casts (same source and target type, including `i32 as isize`).

**Out of scope:**

- Pointer casts (`*const T as *const U`, `usize as *mut T`, etc.) — backend has no `usize`/`u32`/raw-ptr types in `compile_ty`.
- `transmute` — falls under the unsupported-pair branch (no type pair matches); emits the generic `span_err`. No special-casing needed.
- Exotic widths (`u64`, `i128`, `f64`, etc.) — `compile_ty` rejects them first; this spec does not change that.
- Float-to-bool, string-to-number — no node exists; emit a `span_err`.
- Enum-to-int casts — deferred until enums are supported (separate spec).
- New node definitions in `arithmetic.rs` — none needed; we only wire up existing ones.

## Approach

Add one new arm to `compile_assign_rvalue` in `core/src/compile/func.rs`. The arm:

1. Resolves the source `AnyValue` from the operand's MIR type.
2. Resolves the target `AnyValue` from the cast's target MIR type.
3. **Fast path:** if the two `AnyValue`s are equal, forward the operand directly to the destination place (no node inserted).
4. **Slow path:** if the pair is in the supported list, insert `node_convert_type(from, to)`, connect the operand output to the node's single input, then wire the node output to the destination place.
5. **Error:** otherwise emit a `span_err` naming both types and the `CastKind`.

A small private helper `cast_supported(from: &AnyValue, to: &AnyValue) -> bool` enumerates the 11 supported pairs by `ServerTypeId`.

## Components

### Modified: `core/src/compile/func.rs`

**New arm in `compile_assign_rvalue`:**

```rust
Rvalue::Cast(kind, op, target_ty) => {
    // Use the same idiom as the existing compile_assign_rvalue arms (see BinaryOp):
    // op.ty(&self.body.local_decls, self.tcx) returns the Operand's Ty<'tcx>.
    let from_ty = compile_ty(self.compiler, op.span(&self.body.local_decls),
                             op.ty(&self.body.local_decls, self.tcx))?;
    let to_ty   = compile_ty(self.compiler, span, target_ty)?;
    let value_in = if from_ty == to_ty {
        // No-op cast (e.g. i32 as isize). Forward operand.
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
```

This arm replaces the existing `Rvalue::Cast` arm that was part of the trailing `todo!()` bucket.

**New private helper in `core/src/compile/func.rs`:**

```rust
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

### Unchanged

- `core/src/asset/node_graph/arithmetic.rs` — `node_convert_type` and its kernel-ID table are used as-is.
- `core/src/compile/mod.rs` — `compile_ty` is unchanged.
- `core/proto/asset.proto` — no schema changes needed.

## Data flow

For `let y: f32 = x as f32;` where `x: i32`:

1. MIR emits `Assign(_y, Cast(_, _x, f32))`.
2. The new arm compiles: `from_ty = ValueInt::def()`, `to_ty = ValueFloat::def()`.
3. `cast_supported(ValueInt, ValueFloat)` returns `true`.
4. Insert `node_convert_type(ValueInt, ValueFloat)` → kernel ID 181.
5. Wire `_x`'s value output → node input pin 0 via `compile_operand` + `set_value_in`.
6. Compile the result via `compile_assign`, which inserts a `set_local` node and connects the conversion node's output to it.

## Error handling

| Case | Behavior |
|---|---|
| Source == target type | Pass through (no node inserted) |
| Supported pair (the 11 listed in `cast_supported`) | Insert `node_convert_type`, wire it up |
| Unsupported pair | `span_err` at the `as` expression: `"Unsupported cast {from:?} → {to:?} ({kind:?})"` |
| Source or target type rejected by `compile_ty` | The existing `compile_ty` error fires first (e.g. `Unsupported float: f64`); cast logic is not reached |
| `CastKind::Transmute` or pointer casts with no supported mapping | Caught by the "unsupported pair" branch; `span_err` names the `CastKind` |

Errors flow through the existing `Result` machinery in `compile_assign_rvalue`. The compiler session collects them via rustc's diagnostic emitter.

## Testing

There is no automated test harness in the project today. Verification for this spec is build-and-inspect:

**Positive cases** — extend `demo/src/lib.rs` with:

```rust
#[unsafe(no_mangle)]
pub fn cast_i32_to_f32(x: i32) -> f32 { x as f32 }

#[unsafe(no_mangle)]
pub fn cast_f32_to_i32(x: f32) -> i32 { x as i32 }

#[unsafe(no_mangle)]
pub fn cast_bool_to_i32(b: bool) -> i32 { b as i32 }

#[unsafe(no_mangle)]
pub fn cast_i32_to_bool(x: i32) -> bool { (x as i32) != 0 }

// Implicit-exercise cases (no new fn needed; covered by existing arithmetic):
//   `cast_bool_to_i32`  exercises kernel 185 (Bool→Int)
//   `cast_i32_to_f32`   exercises kernel 181 (Int→Float)
//   `cast_f32_to_i32`   exercises kernel 187 (Float→Int)
//   `cast_i32_to_bool`  exercises kernel 180 (Int→Bool), via the `x as i32`
//                       subexpression combined with `!=` — the cast itself
//                       is a no-op (i32→i32), so to actually exercise kernel
//                       180 the demo needs an explicit `i32 as bool`. Adjust
//                       accordingly during implementation.
```

During implementation, the demo should include at least one fn that uses `x as bool` where `x: i32` directly, so kernel 180 is reached.

**Negative case** (manual, not committed): temporarily add a `*const u8 as i32` cast to `cast_i32_to_f32` and confirm the build fails with the expected `span_err` text. Then revert.

**Verification commands:**

```shell
cargo +nightly run -p build-demo
ls -la target/rust2genshin-demo.gia
```

Inspect the `.gia` with `protoc --decode_raw` or a hex dump; confirm:

- `NodeGraphData` for each cast fn contains a node with `runtime_id` in {180, 181, 185, 187} matching the source/target pair.
- No `NodeGraphData` references kernel IDs outside that set (no orphan conversion nodes).

## Risks

- **Equality on `AnyValue`:** the fast path uses `from_ty == to_ty`. `AnyValue` derives or implements equality such that two `ValueInt::def()` instances compare equal; if not, the fast path could be skipped (no correctness regression, just an extra node inserted). Mitigation: confirm equality works in practice; otherwise remove the fast path and let the slow path handle it.
- **Selectortype pin behavior:** `node_convert_type` uses `selectors_in` for the polymorphic input. `set_value_in` writes the default value (or a link); if a future change breaks polymorphism handling, the conversion node may emit wrong-type pins. Out of scope for this spec; flag if observed in testing.
- **Future expansion:** when `unsigned int` and `i64` are added in later specs, the supported-pair list will grow. Keep `cast_supported` next to where the conversion node lives, or move it into `arithmetic.rs` as a `pub fn`.

## Out-of-spec follow-ups

- `unsigned int` (Tier 1 #2): widen `compile_ty` to accept `UintTy::U32`; extend `cast_supported` to handle `u32 ↔ i32`.
- `i64` (Tier 1 #3): widen `compile_ty` for `IntTy::I64`; extend `cast_supported`.
- `tuple` (Tier 1 #4): add tuple→tuple / scalar→tuple casts when tuple support lands.
- `struct` (Tier 2 #5): struct casts will go through a different node (`STRUCT_ASSEMBLY` / `STRUCT_MODIFY`), not `node_convert_type`.