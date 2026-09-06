# Tuple Comparison — Design Spec

**Date:** 2026-09-07
**Status:** Approved (brainstorming complete) — **revised after implementation**: see "Implementation pivot" below. The macro-based approach shipped; the original BinOp::Eq backend approach was attempted and reverted.
**Scope:** Enable `==` and `!=` on 2-tuples in user code via a `tuple_eq!` macro. The macro expands a 2-tuple comparison into field-wise scalar `==` chained with `&&`, so the backend's existing scalar paths do all the work — no backend changes for tuple comparison. Nested 2-tuples are supported by recursing at the source level. Ordering operators (`<`, `<=`, `>`, `>=`) remain out of scope. 3+-tuple equality is not supported by the macro (would silently compare only fields `.0` and `.1` — see "Limitations" below).

## Implementation pivot

The original design proposed a backend-side approach: branch `BinOp::Eq`/`Ne` in `compile_assign_rvalue` on `kind0.downcast_ref::<ValueStruct>()` and route tuple operands through a new `compare_tuple_values` helper that recursively splits both sides via STRUCT_SPLIT, compares each field with `node_equal`, and folds with NODE_AND.

**Why it didn't work:** rustc lowers `p == q` for tuple types NOT to `Rvalue::BinaryOp(Eq, ...)`, but to a trait method call: `_0 = <(A, B) as PartialEq>::eq(move _3, move _4)`. The intermediate locals `_3: &(A, B)` and `_4: &((A, B))` (refs into the args) panic in `solve_local` because `compile_ty` doesn't support `Ref(Tuple)`, and the call to `PartialEq::eq` would route through `compile_call` → `touch_fn` for a stdlib fn, which doesn't work either. The original backend approach was implemented (commits `2ecb8e7`, `6dd3dd0`) and reverted (`b92df0d`, `1356ab9`).

**What shipped:** a `tuple_eq!` macro in `rust2genshin-lib` (commit `813640a`) that expands `tuple_eq!(a, b)` to `a.0 == b.0 && a.1 == b.1`. Each scalar `==` lowers to `Rvalue::BinaryOp(Eq, ...)` (the same path that works for `i32 == i32` etc.), and `&&` lowers to `Rvalue::BinaryOp(BitAnd, bool)` → `NODE_AND`. The backend handles every piece through its existing scalar paths; no backend changes for tuple comparison.

**Side benefit:** commits `a743400` and `b297412` (extracting `insert_struct_split` and migrating `Flat::setter` to it) shipped as planned. Those changes are not on the critical path for tuple comparison but improve the Flat setter's maintainability.

## Context

The original spec (now historical, see "Implementation pivot" above) was written assuming `BinOp::Eq` fires for tuple types. It does not — see the explanation above. The remaining sections of this doc describe the original design; the macro-based approach that actually shipped is fully described in the `tuple_eq!` doc comment in `lib/src/lib.rs` (lines 49–65) and in the commit message for `813640a`.

The `Flat::getter` and `Flat::setter` work landed in commit `979a6c8 fix(core): correct STRUCT_ASSEMBLY/SPLIT pin layout in Flat::getter/setter`. The pin layout for STRUCT_SPLIT (300003) is now: pin 0 in = struct value (polymorphic), pins 0..N out = per-field values, with `selectors_in/out` properly resized.

The demo file already has `make_tuple`, `tuple_first`, `tuple_second`, `nested_tuple_first`, `swap_pair`, `copy_pair`, `update_field`, `nested_update`. None of them compare tuples.

## Scope (revised)

**In scope (shipped):**

1. A `tuple_eq!` macro in `rust2genshin-lib` (`lib/src/lib.rs`) that expands 2-tuple comparison into field-wise scalar `==` chained with `&&`.
2. Two demo functions in `demo/src/lib.rs` exercising the macro: `tuple_eq` (flat `(i32, f32)`) and `nested_tuple_eq` (`((i32, f32), bool)`).
3. Extracted `insert_struct_split` helper in `core/src/compile/func.rs` and migrated `LocalVar::Flat::setter` to use it.

**Out of scope:**

- Backend handling of `<(A, B, ...) as PartialEq>::eq` trait calls — would require modeling references and trait-method detection in `compile_call`; substantially more work.
- Ordering operators (`<`, `<=`, `>`, `>=`) on tuples. Rare in Rust (tuples only implement `PartialOrd`, not `Ord`).
- `LocalVar::Struct` comparison (user-declared structs). Different code path.
- 3+-tuple equality (see "Limitations").

## Limitations

- The macro is fixed-arity at 2 fields. Calling `tuple_eq!` on a 3-tuple compiles but silently compares only fields `.0` and `.1`, producing a wrong answer. A variadic form or explicit `tuple_eq3!` / `tuple_eq4!` macros would fix this; deferred until needed.
- The macro is a 2-tuple helper. For nested tuples like `((A, B), C)`, the user recurses at the source level: `tuple_eq!(p.0, q.0) && p.1 == q.1`. This works as long as the recursion bottoms out at scalar fields (no tuple-in-tuple-in-tuple with no intermediate scalar) — matches the existing `demo/src/lib.rs` style.
- Tuple `==` syntax (`p == q`) still panics at codegen. The backend doesn't model rustc's trait-dispatch lowering. Out of scope; future work would need to intercept the `<(A, B) as PartialEq>::eq` call in `compile_call`.

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

### Approach (revised — macro-based)

The shipped approach has three parts: a macro in the lib, demo functions using the macro, and the `insert_struct_split`/`Flat::setter` refactor (which landed but isn't on the critical path for tuple comparison).

#### Part A — `tuple_eq!` macro in `rust2genshin-lib`

Add to `lib/src/lib.rs`:

```rust
/// Compare two 2-tuples by field, returning a bool.
///
/// Expands `tuple_eq!(a, b)` to `a.0 == b.0 && a.1 == b.1`. The expansion uses
/// only scalar `==` (which MIR lowers to `Rvalue::BinaryOp(Eq, ...)`, not the
/// `<T as PartialEq>::eq` trait dispatch), so the backend's existing scalar
/// comparison paths handle every field. For nested tuples, recurse at the
/// source level:
///
/// ```ignore
/// tuple_eq!(p.0, q.0) && p.1 == q.1   // for ((A, B), C)
/// ```
#[macro_export]
macro_rules! tuple_eq {
    ($a:expr, $b:expr) => {
        ($a).0 == ($b).0 && ($a).1 == ($b).1
    };
}
```

The `$a` and `$b` are parenthesized to avoid parser ambiguity when callers pass expressions like `*x` or `f()`.

#### Part B — Demo functions

Append to `demo/src/lib.rs`:

```rust
#[unsafe(no_mangle)]
pub fn tuple_eq(p: (i32, f32), q: (i32, f32)) -> bool {
    rust2genshin_lib::tuple_eq!(p, q)
}

#[unsafe(no_mangle)]
pub fn nested_tuple_eq(p: ((i32, f32), bool), q: ((i32, f32), bool)) -> bool {
    rust2genshin_lib::tuple_eq!(p.0, q.0) && p.1 == q.1
}
```

`tuple_eq` exercises flat tuple equality. `nested_tuple_eq` exercises the source-level recursion pattern.

#### Part C — `insert_struct_split` refactor (orthogonal)

This change shipped alongside tuple comparison for maintainability reasons but is not on the critical path. The original spec's Changes 1 and 4 (`insert_struct_split` extraction + `Flat::setter` migration) landed as commits `a743400` and `b297412`; the original spec's Changes 2 and 3 (backend `compare_tuple_values` + `BinOp::Eq` branch) landed as `2ecb8e7` and `6dd3dd0` then were reverted (`b92df0d` and `1356ab9`) because rustc's MIR lowering bypasses them entirely.

## Components

### Modified (shipped)

- `core/src/compile/func.rs`:
  - New `insert_struct_split` helper (Part C, commit `a743400`).
  - `LocalVar::Flat::setter` refactored to call `insert_struct_split` (Part C, commit `b297412`).
- `lib/src/lib.rs`:
  - New `tuple_eq!` macro (Part A, commit `813640a`).
- `demo/src/lib.rs`:
  - Two new demo functions (Part B, commit `a777c50`).

### Unchanged

- `core/src/asset/node_graph/arithmetic.rs` — `NODE_SPLIT_STRUCT`, `NODE_AND`, `node_equal` already exist.
- `core/src/asset/value.rs` — `ValueStruct::fields` already carries per-field types.
- `core/src/compile/func.rs` — `LocalVar::Struct` arms remain `todo!()`; `BinOp::Eq | BinOp::Ne => node_equal(kind0)` is unchanged (scalar path only).

## Data flow

### Flat tuple equality `tuple_eq!(p, q)` where `p, q: (i32, f32)`

1. Source after macro expansion: `tuple_eq`'s body becomes `p.0 == q.0 && p.1 == q.1`.
2. MIR: `_3 = p.0`, `_4 = q.0`, `_5 = Eq(_3, _4)` (BinOp::Eq on i32), `_6 = p.1`, `_7 = q.1`, `_8 = Eq(_6, _7)` (BinOp::Eq on f32), `_0 = BitAnd(_5, _8)` (BinOp::BitAnd on bool). No `<(i32, f32) as PartialEq>::eq` trait call.
3. Backend:
   - `_5 = Eq(_3, _4)` → existing `BinOp::Eq` arm in `compile_assign_rvalue` calls `node_equal(ValueInt)` → `node_equal` (arithmetic.rs:377) returns polymorphic EQUAL with kernel 370 for i32.
   - `_8 = Eq(_6, _7)` → `node_equal(ValueFloat)` → kernel 371.
   - `_0 = BitAnd(_5, _8)` → existing `BinOp::BitAnd` arm (compile_assign_rvalue:286) detects `ty.is_bool()` → `NODE_AND`.
4. Result: a bool connection flows into `_0` (the function's return local).

### Nested tuple equality `tuple_eq!(p.0, q.0) && p.1 == q.1` where `p, q: ((i32, f32), bool)`

1. Source after macro expansion: `nested_tuple_eq`'s body becomes `(p.0).0 == (q.0).0 && (p.0).1 == (q.0).1 && p.1 == q.1`.
2. MIR: chain of three scalar `Eq`s and two `BitAnd`s, all on scalar operands. No trait calls.
3. Backend: three `node_equal` (kernel 370, 371, 786) and two `NODE_AND` nodes. Result is a bool.

## Error handling

| Case | Behavior |
|---|---|
| Wrong-arity `tuple_eq!(p, q)` (e.g., 3-tuple) | Silent wrong answer — compiles and produces a node graph that compares only fields `.0` and `.1`. See "Limitations" above. Future work: explicit arity (`tuple_eq3!`, etc.). |
| Macro called with non-2-tuple expression (e.g., a scalar) | Compile error at field access `.0` or `.1` (no such field on the type). |
| Existing demo functions | Unchanged behavior. The `Flat::setter` refactor (Part C) preserves the happy-path behavior identically; defensive `nop` fallback replaces an unreachable broken-VALUE_BOOL placeholder path. |

No new `span_err` paths — error signaling stays consistent with the project's panic-and-`todo!()` convention for unsupported features.

## Testing

No automated test harness for the backend. Verification is build-and-inspect:

1. Run `cargo +nightly build -p rust2genshin` — backend compiles.
2. Run `cargo +nightly run -p build-demo` — full pipeline runs to completion.
3. `target/rust2genshin_demo.gia` is produced. Size grew from 32,108 bytes (pre-feature) to 40,151 bytes (post-feature) — modest growth from the three new scalar `Eq` nodes and one extra `NODE_AND` per demo function.
4. The pipeline must NOT panic on `tuple_eq` or `nested_tuple_eq`. If a panic occurs, the macro expansion or backend integration is broken.
5. **Regression check:** existing demo functions (`make_tuple`, `tuple_first`, `swap_pair`, etc.) must still produce valid output. The `Flat::setter` refactor (Part C) is the regression risk; the build-demo pipeline catches breakage here.

What verification does NOT cover:
- Engine-side correctness (whether the `.gia` actually evaluates to the right bool in the Genshin editor). No engine integration test exists; this is consistent with the project's verification posture.
- 3+ tuples silently produce wrong answers (see "Limitations" — not exercised by the demo).

## Risks

- **Wrong-arity silent failure** — calling `tuple_eq!` on a 3-tuple compiles and produces wrong results, not a compile error. Mitigation: this matches the spec's "out of scope" posture for non-2-tuples; documented in the macro doc comment and in "Limitations" above. Future work: arity-checked variadic macro.
- **`Flat::setter` refactor (Part C) subtly changes behavior** if `insert_struct_split`'s pin layout diverges from the inline code. Mitigation: the helper uses the exact same selector/type assignments as the original inline code. Pipeline regression check catches divergence.
- **Macro doesn't enforce 2-tuple shape** — `tuple_eq!(x, y)` where `x: A` and `y: B` (scalars) compiles if A and B happen to have `.0` and `.1` fields, with nonsense results. The demo never exercises this; not a project risk today.

## Out-of-spec follow-ups

1. **Tuple `==` syntax (`p == q`)** — would require modeling rustc's `<(A, B, ...) as PartialEq>::eq` trait dispatch in the backend (currently the macro sidesteps this by expanding at the source level). Substantial work: references (`&(A, B)`) aren't modeled, and trait method detection in `compile_call` is a new feature.
2. **Arity-checked variadic macro** — `tuple_eq!(p, q; 0, 1, 2)` for 3-tuples, with compile-time arity check.
3. **Tuple ordering (`<`, `<=`, `>`, `>=`)** — lexicographic decomposition with chained conditionals. Rare in Rust.
4. **`LocalVar::Struct` comparison** — user-declared struct comparison via the same macro pattern.
5. **Short-circuit AND** — replace sequential ANDs with conditional AND for performance on large tuples.
