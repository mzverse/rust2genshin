# Flat::getter and Flat::setter — Design Spec

**Date:** 2026-09-07
**Status:** Approved (brainstorming complete)
**Scope:** Implement `LocalVar::Flat::getter()` and `LocalVar::Flat::setter()` in `core/src/compile/func.rs` using `NODE_ASSEMBLE_STRUCT` (kernel 300002) and `NODE_SPLIT_STRUCT` (kernel 300003). After this spec, whole-tuple reads (`let p = t;`) and whole-tuple writes (`p = t;`) work end-to-end, enabling user code like `let mut p = t; p.0 = x; p = q;`.

## Context

The current `LocalVar` enum (introduced by commit `b5f82b0 refactor(core): flat tuple`) has three variants:
- `LocalVar::Basic(NodeRef)` — scalar local.
- `LocalVar::Struct { node: NodeRef, getter: NodeRef }` — user struct (currently `todo!()`).
- `LocalVar::Flat(IndexVec<FieldIdx, LocalVar>)` — recursively flattened tuple (e.g., `(i32, (f32, bool))` becomes `Flat([Basic, Flat([Basic, Basic])])`).

The `Basic` variant has working `getter()` (returns `Connection(node, 1)`) and `setter()` (inserts `node_set_local`). The `Struct` and `Flat` variants both `todo!()`. This blocks any code path that calls `getter()` or `setter()` directly on a `Flat` — most notably, `let p = t;` (whole-tuple move).

`compile_assign` (line 157 in `core/src/compile/func.rs`) walks the destination's projection chain to a leaf and then calls `local.setter(...)`. For `p.0 = x` (field write), the walk lands on a `Basic` leaf and the existing setter works. For `let p = t;` (no projection), the walk doesn't iterate, `local` stays `Flat`, and `Flat::setter()` panics.

The `NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` node constants already exist in `core/src/asset/node_graph/arithmetic.rs:654-662` from earlier sub-projects. They are unused dead code in the current refactor.

This spec reuses those constants to implement `Flat::getter()` and `Flat::setter()` properly.

## Scope

**In scope:**

1. `LocalVar::Flat::getter(&self, graph, kind) -> ValueIn` — implement using STRUCT_ASSEMBLY.
2. `LocalVar::Flat::setter(&self, graph, kind, value) -> Block` — implement using STRUCT_SPLIT.
3. `LocalVar::Struct::getter()` and `LocalVar::Struct::setter()` — still `todo!()`. Out of scope for this sub-project.
4. Update `compile_assign` to pass `graph` and `kind` to `getter()` / `setter()` (signature change for `getter`).
5. Update `compile_operand` to call `getter(graph, kind)` (returns `ValueIn` instead of `Connection`).
6. Add 4 demo functions to `demo/src/lib.rs` exercising whole-tuple moves and field writes.

**Out of scope:**

- `LocalVar::Struct` (user-declared structs). Still `todo!()`. Future sub-project.
- `compile_ty` for tuples — currently `todo!()` (reverted in commit `b5f82b0`). Not needed for this sub-project because the Flat path doesn't call `compile_ty` directly; field types come from `ValueStruct.fields` resolved at runtime via `downcast_ref`.
- Tuple comparison (`(a, b) == (c, d)`) — separate sub-project.

## Approach

### Change 1 — `LocalVar::Flat::getter`

Open `core/src/compile/func.rs`. Find the `impl LocalVar` block. Replace the `Flat` arm of `getter()`:

```rust
LocalVar::Flat(_) => todo!(),
```

with:

```rust
LocalVar::Flat(fields) => {
    // Insert STRUCT_ASSEMBLY (kernel 300002): collects per-field leaf getters
    // into a single struct value. The struct_id selector (input pin 0) is
    // set from `kind`'s ValueStruct.struct_id. The dynamic field inputs
    // (pins 1..N+1) come from each leaf's getter().
    let mut node_kind = crate::asset::node_graph::arithmetic::NODE_ASSEMBLE_STRUCT.clone();
    // Populate input types: struct_id selector (int) + N leaf types.
    let field_types: Vec<AnyValue> = if let Some(s) = kind.as_ref().downcast_ref::<ValueStruct>() {
        s.fields.clone()
    } else {
        return ValueIn::value(ValueBool::def()); // defensive: not a struct
    };
    node_kind.values_in_types = std::iter::once(ValueInt(0).into())
        .chain(field_types.iter().cloned())
        .collect();
    // Set the output type to `kind` itself.
    node_kind.values_out_types = vec![kind.clone()];
    let struct_id = if let Some(s) = kind.as_ref().downcast_ref::<ValueStruct>() {
        s.struct_id
    } else {
        0
    };
    let node_ref = graph.insert(node_kind);
    // Set struct_id selector on input pin 0.
    graph.set_value_in(
        Connection(node_ref, 0),
        ValueIn::value(ValueInt(struct_id as i32).into()),
    );
    // Wire each leaf's getter to the corresponding field input pin.
    for (i, field) in fields.iter().enumerate() {
        let leaf_value = field.getter(graph, /* kind */ ValueBool::def()); // leaf kind is scalar; pass any
        graph.set_value_in(Connection(node_ref, (i + 1) as i32), leaf_value);
    }
    ValueIn::link(Connection(node_ref, 0).into())
}
```

(The leaf `kind` passed to `getter()` is a placeholder; `Basic::getter()` ignores `kind`. The setter path is more interesting.)

### Change 2 — `LocalVar::Flat::setter`

Replace the `Flat` arm of `setter()`:

```rust
LocalVar::Flat(_) => todo!(),
```

with:

```rust
LocalVar::Flat(fields) => {
    // Insert STRUCT_SPLIT (kernel 300003): takes the struct value and produces
    // per-field outputs. For each leaf, insert a set_local and wire the
    // STRUCT_SPLIT's per-field output to the leaf's set_local.
    let mut node_kind = crate::asset::node_graph::arithmetic::NODE_SPLIT_STRUCT.clone();
    let field_types: Vec<AnyValue> = if let Some(s) = kind.as_ref().downcast_ref::<ValueStruct>() {
        s.fields.clone()
    } else {
        return Block::nop(graph);
    };
    node_kind.values_in_types = vec![kind.clone()];
    node_kind.values_out_types = field_types.clone();
    let node_ref = graph.insert(node_kind);
    // Wire the value to the struct input.
    graph.set_value_in(Connection(node_ref, 0), value);
    // For each leaf, insert a set_local and wire STRUCT_SPLIT's output to it.
    let mut block = Block::nop(graph);
    for (i, field) in fields.iter().enumerate() {
        let leaf_kind = &field_types[i];
        let block_for_field = field.setter(graph, leaf_kind.clone(), ValueIn::link(Connection(node_ref, i).into()));
        block.extend(graph, block_for_field);
    }
    block
}
```

(Note: setter's signature stays the same — `(&self, graph, kind, value) -> Block`. The change is implementing the `Flat` arm using STRUCT_SPLIT.)

### Change 3 — `compile_operand` signature change

The current `compile_operand`:

```rust
Operand::Copy(p) |
Operand::Move(p) => {
    let mut local = self.locals.get(p.local).unwrap();
    for e in p.projection {
        match e {
            PlaceElem::Field(i, _) => {
                match local {
                    LocalVar::Basic(_) => unreachable!(),
                    LocalVar::Struct { .. } => todo!("struct"),
                    LocalVar::Flat(v) => local = v.get(i).unwrap(),
                }
            }
            _ => todo!("{e:?}"),
        }
    }

    ValueIn::link(local.getter().into())
}
```

Replace `ValueIn::link(local.getter().into())` with `local.getter(self.graph, /* kind */ /* ... */)` — `getter()` now takes `graph` and `kind`. The `kind` for the read is the leaf's type (for Basic) or the struct type (for Flat). For correctness, the source's `kind` is needed; derive it from the source's `place.ty()`.

Replace with:

```rust
let src_kind = self.compiler.compile_ty(span, p.ty(&self.body.local_decls, self.tcx))?;
local.getter(self.graph, src_kind)
```

### Change 4 — `compile_assign` setter call

The current `compile_assign`:

```rust
Ok(local.setter(self.graph, kind, value))
```

This already takes `self.graph` and `kind`. The signature of `Basic::setter` is `(&self, graph, kind, value) -> Block`. The signature change is internal to `Flat::setter`. No change needed at the call site.

### Change 5 — Add 4 demo functions

In `demo/src/lib.rs`, append:

```rust
#[unsafe(no_mangle)]
pub fn swap_pair(p: (i32, f32)) -> (i32, f32) {
    let mut pair = p;
    let tmp = pair.0;
    pair.0 = pair.1 as i32;
    pair.1 = tmp as f32;
    pair
}

#[unsafe(no_mangle)]
pub fn copy_pair(p: (i32, f32)) -> (i32, f32) {
    let copy = p;
    copy
}

#[unsafe(no_mangle)]
pub fn update_field(p: (i32, f32), v: i32) -> (i32, f32) {
    let mut pair = p;
    pair.0 = v;
    pair
}

#[unsafe(no_mangle)]
pub fn nested_update(p: ((i32, f32), bool), n: i32) -> ((i32, f32), bool) {
    let mut pair = p;
    pair.0.0 = n;
    pair
}
```

These exercise:
- `swap_pair`: whole-tuple move + multiple field writes
- `copy_pair`: whole-tuple move (read + write)
- `update_field`: whole-tuple move + single field write
- `nested_update`: whole-tuple move + nested field write (`pair.0.0`)

## Components

### Modified

- `core/src/compile/func.rs`:
  - `LocalVar::Flat::getter` implementation (new STRUCT_ASSEMBLY logic).
  - `LocalVar::Flat::setter` implementation (new STRUCT_SPLIT logic).
  - `compile_operand` — call new `getter(graph, kind)` signature.

### Unchanged

- `core/src/asset/node_graph/arithmetic.rs` — `NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` exist; no changes.
- `core/src/asset/value.rs` — `ValueStruct` already encodes `struct_id` correctly.
- `core/src/compile/func.rs` — `LocalVar::Struct` remains `todo!()`.

## Data flow

### Whole-tuple read (`let copy = p;` where p: (i32, f32))

1. MIR: `Assign(_copy, Use(Operand::Move(Place { local: _p, projection: [] })))`.
2. `compile_assign_rvalue`'s `Rvalue::Use` arm calls `compile_operand(Move(_p))`.
3. `compile_operand` walks: `local = Flat([Basic(i32), Basic(f32)])`. No projection to iterate. `local` stays `Flat`.
4. `compile_operand` calls `local.getter(graph, kind)` where `kind = (i32, f32)`.
5. `Flat::getter` inserts STRUCT_ASSEMBLY, wires each leaf's getter to a per-field input, returns `ValueIn::link(Connection(asm_node, 0))`.
6. `compile_assign(place=_copy, value_in)` writes the value to `_copy`. With empty projection, walks to `_copy`'s Flat, calls `Flat::setter(graph, kind, value_in)`.
7. `Flat::setter` inserts STRUCT_SPLIT on the value's struct input, routes each per-field output to a set_local on the corresponding leaf.

### Field write (`p.0 = x;`)

1. MIR: `Assign(_p.0, x)`.
2. `compile_assign` walks projection `[Field(0)]`: `local = Flat([Basic(i32), Basic(f32)])` → walk Field(0) → `local = Basic(i32)`.
3. `kind = i32` (derived from `place.ty()`).
4. `Basic::setter(graph, i32, value)` — inserts set_local node (existing path, works).
5. `value` is the operand's `ValueIn` (the `x` value).

### Nested field write (`p.0.0 = n;`)

1. MIR: `Assign(_p.0.0, n)`.
2. `compile_assign` walks projection `[Field(0), Field(0)]`:
   - Start: `local = Flat([Flat([Basic(i32), Basic(f32)]), Basic(bool)])`.
   - Walk Field(0): `local = Flat([Basic(i32), Basic(f32)])`.
   - Walk Field(0): `local = Basic(i32)`.
3. `kind = i32` (leaf type).
4. `Basic::setter` — works.

## Error handling

| Case | Behavior |
|---|---|
| `kind` is not a struct | `Flat::getter` returns `ValueIn::value(ValueBool::def())` (defensive). `Flat::setter` returns `Block::nop(graph)`. |
| Out-of-bounds field index in projection walk | Current path: `unreachable!()` (compile-time panic, never triggered). |
| `LocalVar::Struct` getter/setter | Still `todo!()` — out of scope. |

## Testing

There is no automated test harness for the backend. Verification is build-and-inspect:

1. The 4 demo functions above are added to `demo/src/lib.rs`.
2. Run `cargo +nightly run -p build-demo` — expect success.
3. Inspect `target/rust2genshin_demo.gia`: each new composite node has a `result.field_0` and `result.field_1` (since the demo functions all return `(i32, f32)`).
4. `cargo +nightly test -p rust2genshin` — 5/5 pass.
5. `cargo +nightly clippy --workspace --all-targets` — only environmental warnings (3 baseline).

## Risks

- **`kind` field types must match the leaf types.** If `kind`'s `ValueStruct.fields[i]` doesn't match the actual `LocalVar::Flat.fields[i]` (e.g., due to a bug in `solve_local`), the wire format will be inconsistent and the node graph will be invalid. Mitigation: the recursive flattening in `solve_local` ensures both come from the same MIR type.
- **`NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` define empty input/output types** that we populate at insertion time. If the actual node kernel expects different field-id or selector semantics, the wire format may be wrong. Mitigation: these constants were previously exercised in the cast/struct sub-projects and worked correctly with `ValueInt(struct_id)` selectors.
- **`set_value_in` on `Connection(node_ref, 0)` for STRUCT_SPLIT's struct input** — the input pin 0 is the polymorphic selector. We pass the struct-typed `value` directly. If the engine expects a different type for that pin (e.g., a value-instead-of-link), it could fail at encode time. Mitigation: previous sub-projects used this pattern and it worked.

## Out-of-spec follow-ups

After this sub-project:

1. **Tuple comparison `(a, b) == (c, d)`** — requires wiring `node_equal` for struct types.
2. **`LocalVar::Struct` getter/setter** — user-declared struct locals.
3. **Removing dead STRUCT_ASSEMBLY/SPLIT declarations** if either is unused after this sub-project.
4. **Polymorphic-selector markers** (`selectors_in[0] = Some(0)`, etc.) for STRUCT_ASSEMBLY — may be needed if the engine requires explicit selector configuration.