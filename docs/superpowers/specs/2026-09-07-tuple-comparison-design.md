# Tuple Comparison — Design Spec

**Date:** 2026-09-07
**Status:** Approved (brainstorming complete)
**Scope:** Enable `==` and `!=` on tuple types in user code (`let eq = (a, b) == (c, d);`). After this spec, tuples compare element-wise with the result combined via AND, for both flat tuples (e.g., `(i32, f32)`) and nested tuples (e.g., `((i32, f32), bool)`). Ordering operators (`<`, `<=`, `>`, `>=`) remain out of scope.

## Context

The existing `BinOp::Eq` and `BinOp::Ne` arms in `compile_assign_rvalue` (line 290 of `core/src/compile/func.rs`) build a single comparison node via:

```rust
BinOp::Eq | BinOp::Ne => node_equal(kind0),
```

where `kind0 = compile_ty(lhs.ty)`. For scalar types, `node_equal` (in `core/src/asset/node_graph/arithmetic.rs:377`) returns a polymorphic EQUAL node whose kernel is selected by the value's server type (Int→370, Flt→371, Bol→786, etc.). For tuple types, `compile_ty` returns a `ValueStruct`, but `node_equal` panics on any type outside its 9-element supported set (the panic at line 399 of `arithmetic.rs`). So today, `let eq = (a, b) == (c, d);` panics at codegen time.

The `Flat::getter` and `Flat::setter` work landed in commit `979a6c8 fix(core): correct STRUCT_ASSEMBLY/SPLIT pin layout in Flat::getter/setter`. The pin layout for STRUCT_SPLIT (300003) is now: pin 0 in = struct value (polymorphic), pins 0..N out = per-field values, with `selectors_in/out` properly resized. This same pin-layout pattern is what tuple comparison needs to invoke externally.

The demo file already has `make_tuple`, `tuple_first`, `tuple_second`, `nested_tuple_first`, `swap_pair`, `copy_pair`, `update_field`, `nested_update`. None of them compare tuples.

## Scope

**In scope:**

1. `BinOp::Eq` and `BinOp::Ne` arms of `compile_assign_rvalue` — branch on `kind0.downcast_ref::<ValueStruct>()` to route tuple operands to a decomposition path; existing scalar path unchanged.
2. New helper `compare_tuple_values(graph, lhs, rhs, &ValueStruct) -> ValueIn` that recursively splits both operands and combines per-field equality via NODE_AND.
3. New helper `insert_struct_split(graph, &ValueStruct, value) -> NodeRef` that builds a properly-pinned STRUCT_SPLIT node — extracted from `Flat::setter` and reused by `compare_tuple_values`.
4. `Flat::setter` refactored to call `insert_struct_split` — keeps the pin-layout construction in one place.
5. Add 2 demo functions to `demo/src/lib.rs`: `tuple_eq` (flat) and `nested_tuple_eq` (nested).

**Out of scope:**

- Ordering operators (`<`, `<=`, `>`, `>=`) on tuples. Rare in Rust (tuples only implement `PartialOrd`, not `Ord`); would require lexicographic decomposition with short-circuit semantics. Possible future sub-project.
- `if let` / `match` on tuple patterns. Already out of scope in the project's overall design (no destructuring support).
- User-declared struct comparison (`struct Foo { ... }; foo1 == foo2`). Different code path — `LocalVar::Struct` is still `todo!()`.
- New kernels. Decomposition uses only existing primitives (`NODE_SPLIT_STRUCT` = 300003, `node_equal` per-field, `NODE_AND`).
- Field types outside `node_equal`'s 9-element supported set (`{Str, Gid, Ety, Vec, Int, Flt, Cfg, Pfb, Bol}`). They propagate the existing panic; not a new error path.

## Approach

### Change 1 — Extract `insert_struct_split` helper

Add to `core/src/compile/func.rs` (next to `flat_child_kinds`):

```rust
/// Insert a STRUCT_SPLIT (kernel 300003) sized for `struct_kind.fields.len()`.
/// Pin layout:
///   - input pin 0 = struct value (polymorphic ValueStruct)
///   - output pins 0..N-1 = per-field values (typed per `struct_kind.fields`)
/// The struct input is wired from `value`. Returns the NodeRef; the caller
/// consumes per-field outputs via `Connection(node, i)`.
fn insert_struct_split(
    graph: &mut NodeGraph<impl NodeGraphExtra>,
    struct_kind: &ValueStruct,
    value: ValueIn,
) -> NodeRef {
    let mut node_kind = crate::asset::node_graph::arithmetic::NODE_SPLIT_STRUCT.clone();
    node_kind.values_in_types = vec![AnyValue::from(struct_kind.clone())];
    node_kind.values_out_types = struct_kind.fields.clone();
    // `NodeKind::new` sized selectors_in/selectors_out from the prototype's
    // (empty) values_*, so resize both in lock-step (see Flat::setter).
    node_kind.selectors_in = vec![None; node_kind.values_in_types.len()];
    node_kind.selectors_in[0] = Some(0);
    node_kind.selectors_out = vec![None; node_kind.values_out_types.len()];
    let node_ref = graph.insert(node_kind.into());
    graph.set_value_in(Connection(node_ref, 0), value);
    node_ref
}
```

### Change 2 — Add `compare_tuple_values` helper

Add to `core/src/compile/func.rs`:

```rust
/// Compare two struct-shaped values field-by-field, returning a bool `ValueIn`.
/// Recursively handles nested tuples (when a field is itself a ValueStruct).
/// Uses STRUCT_SPLIT to decompose both sides, `node_equal` for each leaf pair,
/// and NODE_AND to fold the per-field bools.
fn compare_tuple_values(
    graph: &mut NodeGraph<impl NodeGraphExtra>,
    lhs: ValueIn,
    rhs: ValueIn,
    struct_kind: &ValueStruct,
) -> ValueIn {
    let lhs_split = insert_struct_split(graph, struct_kind, lhs);
    let rhs_split = insert_struct_split(graph, struct_kind, rhs);

    let mut field_results: Vec<ValueIn> = Vec::with_capacity(struct_kind.fields.len());
    for (i, field_kind) in struct_kind.fields.iter().enumerate() {
        let lhs_field = ValueIn::link(Connection(lhs_split, i).into());
        let rhs_field = ValueIn::link(Connection(rhs_split, i).into());
        let field_eq = match field_kind.downcast_ref::<ValueStruct>() {
            Ok(nested) => compare_tuple_values(graph, lhs_field, rhs_field, nested),
            Err(_) => {
                let eq_node = graph.insert(Node::new(node_equal(field_kind.clone())));
                graph.set_value_in(Connection(eq_node, 0), lhs_field);
                graph.set_value_in(Connection(eq_node, 1), rhs_field);
                ValueIn::link(Connection(eq_node, 0).into())
            }
        };
        field_results.push(field_eq);
    }

    // Fold with NODE_AND. Seed with the first result; AND each subsequent.
    let mut combined = field_results[0].clone();
    for next in &field_results[1..] {
        let and_node = graph.insert(Node::new(NODE_AND.clone()));
        graph.set_value_in(Connection(and_node, 0), combined);
        graph.set_value_in(Connection(and_node, 1), next.clone());
        combined = ValueIn::link(Connection(and_node, 0).into());
    }
    combined
}
```

### Change 3 — Branch in `BinOp::Eq`/`Ne` arm

In `core/src/compile/func.rs`, modify the existing `BinOp::Eq | BinOp::Ne => node_equal(kind0),` to:

```rust
BinOp::Eq | BinOp::Ne => {
    if let Ok(vs) = kind0.downcast_ref::<ValueStruct>() {
        // Tuple path: decompose both operands, combine per-field == via AND.
        let lhs_v = self.compile_operand(&v.0, span)?;
        let rhs_v = self.compile_operand(&v.1, span)?;
        compare_tuple_values(self.graph, lhs_v, rhs_v, vs)
    } else {
        node_equal(kind0)
    }
},
```

The existing `if matches!(op, BinOp::Ne)` block that wraps the result in `NODE_NOT` continues to work unchanged — `compare_tuple_values` returns a bool `ValueIn`, which is what `NODE_NOT` consumes.

### Change 4 — Refactor `Flat::setter` to use `insert_struct_split`

Replace the existing inline STRUCT_SPLIT construction in `LocalVar::Flat::setter`:

```rust
let mut node_kind = NODE_SPLIT_STRUCT.clone();
let field_types = flat_child_kinds(&*kind, fields.len());
node_kind.values_in_types = vec![kind.clone()];
node_kind.values_out_types = field_types.clone();
node_kind.selectors_in = vec![None; node_kind.values_in_types.len()];
node_kind.selectors_in[0] = Some(0);
node_kind.selectors_out = vec![None; node_kind.values_out_types.len()];
let node_ref = graph.insert(node_kind.into());
graph.set_value_in(Connection(node_ref, 0), value);
```

with:

```rust
let struct_kind = match kind.downcast_ref::<ValueStruct>() {
    Ok(vs) => vs.clone(),
    Err(_) => {
        // Defensive: Flat::setter is only invoked when the caller passes a
        // tuple type. If we somehow get a non-struct `kind`, fall back to
        // nop to avoid panicking inside the helper.
        return Block::nop(graph);
    }
};
let field_types = struct_kind.fields.clone();
let node_ref = insert_struct_split(graph, &struct_kind, value);
```

Note: this is a small refactor — the existing `Flat::setter` already has correct pin-layout logic, but having the construction in two places is a maintenance hazard. The refactor preserves behavior identically.

### Change 5 — Add 2 demo functions

Append to `demo/src/lib.rs`:

```rust
#[unsafe(no_mangle)]
pub fn tuple_eq(p: (i32, f32), q: (i32, f32)) -> bool {
    p == q
}

#[unsafe(no_mangle)]
pub fn nested_tuple_eq(p: ((i32, f32), bool), q: ((i32, f32), bool)) -> bool {
    p == q
}
```

`tuple_eq` exercises flat tuple equality. `nested_tuple_eq` exercises recursion (the outer split produces a nested-struct field for index 0; the recursion splits it again).

## Components

### Modified

- `core/src/compile/func.rs`:
  - New `insert_struct_split` helper.
  - New `compare_tuple_values` helper.
  - `BinOp::Eq | BinOp::Ne` arm branches on `ValueStruct`.
  - `LocalVar::Flat::setter` refactored to call `insert_struct_split`.

### Unchanged

- `core/src/asset/node_graph/arithmetic.rs` — `NODE_SPLIT_STRUCT`, `NODE_AND`, `node_equal` already exist.
- `core/src/asset/value.rs` — `ValueStruct::fields` already carries per-field types.
- `core/src/compile/func.rs` — `LocalVar::Struct` arms remain `todo!()`.

### Added

- `demo/src/lib.rs` — 2 demo functions.

## Data flow

### Flat tuple equality `(a, b) == (c, d)`

1. MIR: `Assign(_eq, BinaryOp(Eq, (Move(_p), Move(_q))))`.
2. `compile_assign_rvalue` reaches the `BinOp::Eq | BinOp::Ne` arm. `kind0 = compile_ty((i32, f32)) = ValueStruct { struct_id: 0, fields: [ValueInt, ValueFloat] }`. The `downcast_ref` check succeeds.
3. Compile operands: `lhs_v = compile_operand(Move(_p))` returns the STRUCT_ASSEMBLY output link from `_p`'s `Flat::getter` call. Same for `rhs_v` from `_q`.
4. `compare_tuple_values(graph, lhs_v, rhs_v, &vs)`:
   - `insert_struct_split` for lhs → `lhs_split` (pins 0..1 out: i32, f32).
   - `insert_struct_split` for rhs → `rhs_split`.
   - Field 0 (i32): `node_equal(ValueInt)` → `eq_a`.
   - Field 1 (f32): `node_equal(ValueFloat)` → `eq_b`.
   - `eq_a AND eq_b` → combined.
5. `value_in = combined` (bool). The post-Eq `BinOp::Ne` check skips (not Ne). Returns `value_in` from the match.
6. `self.compile_assign(place=_eq, value_in)` — the bool is stored in `_eq` via `LocalVar::Basic::setter` (since the return local for a bool function is a `Basic` leaf).

### Nested tuple equality `((a, b), c) == ((a, b), c)`

1. `kind0 = ValueStruct { fields: [ValueStruct{(i32, f32)}, ValueBool] }`.
2. Outer `compare_tuple_values`:
   - Outer split → `outer_lhs`, `outer_rhs`.
   - Field 0 (ValueStruct): recursive `compare_tuple_values` on the inner struct pair:
     - Inner split → `inner_lhs`, `inner_rhs`.
     - Inner field 0 (i32): `node_equal(ValueInt)` → `eq_a`.
     - Inner field 1 (f32): `node_equal(ValueFloat)` → `eq_b`.
     - `eq_a AND eq_b` → `inner_result`.
   - Field 1 (bool): `node_equal(ValueBool)` → `eq_c`.
   - `inner_result AND eq_c` → combined.

## Error handling

| Case | Behavior |
|---|---|
| `kind0.downcast_ref::<ValueStruct>()` returns `Ok` but `kind0.fields` is empty | `compare_tuple_values` would call `field_results[0]` on an empty Vec — panics. Unreachable in practice: `is_unit()` (`compile/mod.rs:138-147`) filters out 0-tuples before `compile_ty` runs. |
| Field type not in `node_equal`'s 9-element supported set | `node_equal(field_kind)` panics at `arithmetic.rs:399`. Surfaces as a `rustc-ice`, matching existing convention for unsupported types (e.g., `todo!()` for `TyKind::Closure`, `TyKind::Slice`, etc.). |
| Field type IS `ValueStruct` but the inner `compile_ty` didn't produce a ValueStruct | Recursive `compare_tuple_values` panics on `field_results[0]` of an empty Vec. Unreachable: `compile_ty`'s tuple arm always produces a non-empty `ValueStruct`. |
| `Flat::setter` receives a non-struct `kind` (defensive) | Returns `Block::nop(graph)`. Already-degraded behavior is preferable to a panic inside the helper. |

No new `span_err` paths — error signaling stays consistent with the project's panic-and-`todo!()` convention for unsupported features.

## Testing

No automated test harness for the backend. Verification is build-and-inspect:

1. Run `cargo +nightly build -p rust2genshin` — backend compiles.
2. Run `cargo +nightly run -p build-demo` — full pipeline runs to completion.
3. `target/rust2genshin_demo.gia` is produced. Spot-check size vs prior commit (32,108 bytes pre-spec) — expect modest growth from the new decomposition nodes.
4. The pipeline must NOT panic on `tuple_eq` or `nested_tuple_eq`. If a panic occurs, the spec's decomposition logic is broken.
5. **Regression check:** existing demo functions (`make_tuple`, `tuple_first`, `swap_pair`, etc.) must still produce valid output. The `Flat::setter` refactor (Change 4) is the regression risk; the build-demo pipeline catches breakage here.

What verification does NOT cover:
- Engine-side correctness (whether the `.gia` actually evaluates to the right bool in the Genshin editor). No engine integration test exists; this is consistent with the project's verification posture.
- Field-type support beyond `node_equal`'s 9-element set. Surfaced by panic on user code, not by demo.

## Risks

- **`Flat::setter` refactor (Change 4) subtly changes behavior** if `insert_struct_split`'s pin layout diverges from the inline code. Mitigation: the helper uses the exact same selector/type assignments as the original inline code; the spec documents the equivalence. Pipeline regression check catches divergence.
- **NODE_AND is not short-circuiting in this implementation** — all `N` field equality nodes are computed regardless of intermediate results. For a 2-tuple this doesn't matter; for hypothetical large tuples it could be wasteful. Mitigation: tuples in this project are typically 2–4 fields; if large tuples become common, short-circuit decomposition is a future optimization.
- **`node_equal` selector indices** are set in `arithmetic.rs:402-403` (selectors_in[0] and [1]). These index the 9-element type list. For tuple decomposition, the per-field call uses scalar field kinds that ARE in the 9-element set, so the selector assignment works correctly. No risk.
- **Recursive `compare_tuple_values` stack depth** — for a deeply nested tuple (e.g., `(((a,),),)`), each level adds a stack frame. Rust's default stack is 8 MB; even pathological nesting (depth 1000) would consume <100 KB. No risk.

## Out-of-spec follow-ups

1. **Tuple ordering (`<`, `<=`, `>`, `>=`)** — lexicographic decomposition with chained conditionals.
2. **`LocalVar::Struct` comparison** — once user struct locals work, comparison via the same decomposition pattern, but the field-type lookup needs `LocalVar::Struct`'s `getter` to be implemented first.
3. **Short-circuit AND** — replace sequential ANDs with conditional AND for performance on large tuples.
4. **Per-field `node_equal` kernel caching** — for repeated comparisons of the same field type, cache the node-id mapping. Today's implementation creates fresh nodes each time. Probably not worth it for typical use.
