# Tuple Support — Design Spec

**Date:** 2026-09-05
**Status:** Approved (brainstorming complete)
**Scope:** Implement full tuple type support in the MIR→node-graph backend. Tuples map onto genshin `SStruct` with auto-generated struct schemas. After this spec, `compile_ty` accepts non-empty tuple types, `Rvalue::Aggregate(AggregateKind::Tuple, _)` constructs tuples via `STRUCT_ASSEMBLY` nodes, and `Place.projection[Field(i)]` accesses tuple fields via `STRUCT_SPLIT`.

## Context

The `rust2genshin` codegen backend translates Rust MIR into Genshin node-graph assets. Several MIR constructs are still unimplemented; one of them is `TyKind::Tuple`. Currently:

- `compile_ty` panics: `TyKind::Tuple(tys) => todo!("Todo tuple: {:?}", tys)` (`core/src/compile/mod.rs:210`).
- `Rvalue::Aggregate` (which includes tuple construction) panics: in the `todo!()` bucket at `core/src/compile/func.rs:163`.
- `compile_assign` and `compile_operand` reject non-empty `Place.projection` with `todo!()` (`core/src/compile/mod.rs:354` inline TODO, `func.rs:178` for operand copy/move).

This is Tier 1 #4 in the cast spec's follow-up list. The Genshin engine does **not** have a dedicated tuple type — `SStruct = 25` (`core/proto/asset.proto:251`) is the closest match, and the engine provides `STRUCT_ASSEMBLY` (kernel id 300002) and `STRUCT_SPLIT` (kernel id 300003) for construction/deconstruction. Tuples are therefore mapped onto `SStruct` with auto-generated struct schemas. The `StructureDefinition` machinery is already in place (`core/src/asset/node_graph/structure.rs`) but no compiler-side wiring exists yet.

The user explicitly chose "Full tuple support (struct-mapped)" — addressing compile_ty, Aggregate, and field access in one sub-project.

## Scope

**In scope:**

1. `compile_ty` returns a `ValueStruct` for non-empty `TyKind::Tuple` types.
2. A new `tuple_schemas: HashMap<TupleKey, i64>` cache on `Compiler` maps each unique tuple type to a struct_id.
3. A new helper `intern_tuple_schema(ty: Ty) -> Result<i64>` generates a `StructureDefinition` on first encounter, inserts it into `self.assets`, and caches the resulting id.
4. `Rvalue::Aggregate(AggregateKind::Tuple, fields)` is wired in `compile_assign_rvalue` to insert a `STRUCT_ASSEMBLY` node.
5. `Place.projection[ProjectionElem::Field(i, _)]` is wired in `compile_assign` and `compile_operand` to insert a `STRUCT_SPLIT` node and route field `i`.
6. Tuple→tuple casts (`Rvalue::Cast` with tuple source and target) are identity when source and target resolve to the same struct_id; otherwise `span_err`.

**Out of scope (deferred to other sub-projects):**

- **Pattern destructuring** `let (a, b) = tup` — relies on `Place.projection[Field(i)]`, which is in scope; the destructuring MIR form is `Rvalue::Aggregate` (covered) and field reads (covered).
- **Tuple equality `(a, b) == (c, d)`** — requires a tuple-equal node which doesn't exist in `node_graph`. Defer until a tuple-equal node is identified or a composite workaround is designed.
- **Tuple element assignment `tup.0 = 42`** — relies on field projection in `compile_assign`, which becomes possible after this spec. May need STRUCT_MODIFY (id 300004 in `execution.rs:1000`) rather than STRUCT_SPLIT for writes; defer the design.
- **Nested tuples containing references or unsupported types** — already rejected by `compile_ty` for the inner types.
- **Tuples as generic arguments** — generic support is a separate Tier 2 feature.

## Approach

Five mechanical changes to the compiler, each independent of the others except for the dependency ordering (struct cache must exist before `compile_ty` can use it; field access must exist before destructuring works).

### Change 1 — `Compiler` cache and `intern_tuple_schema` helper

Add a field to `Compiler<'tcx>` (in `core/src/compile/mod.rs`):

```rust
tuple_schemas: HashMap<TupleKey, i64>,
```

`TupleKey` is a `Hash`-able wrapper around the canonicalized tuple MIR type. Using a key derived from `(usize /* arity */, Vec<AnyValue /* element types' debug IDs */>)` is simple and avoids deep MIR types in the cache. The `AnyValue` debug-print string (via `format!("{:?}", value)`) is a stable-enough identifier for hashing purposes (any two equal `AnyValue`s produce the same debug string).

`intern_tuple_schema` lives on `Compiler<'tcx>`:

```rust
fn intern_tuple_schema(&mut self, span: Span, ty: Ty<'tcx>) -> Result<i64> {
    let TyKind::Tuple(elem_tys) = ty.kind() else {
        return self.span_err(span, "intern_tuple_schema called with non-tuple type");
    };
    let elem_kinds: Vec<AnyValue> = elem_tys.iter()
        .map(|t| self.compile_ty(span, t))
        .collect::<Result<_>>()?;
    let key = (elem_kinds.len(), format!("{:?}", elem_kinds));
    if let Some(&id) = self.tuple_schemas.get(&key) {
        return Ok(id);
    }
    // Generate the StructureDefinition
    let fields: Vec<StructField> = elem_kinds.iter().enumerate()
        .map(|(i, k)| StructField {
            name: format!("field_{i}"),
            value: k.clone(),
            is_set: false,
        })
        .collect();
    let name = format!("Tuple({})", elem_kinds.iter()
        .map(|k| format!("{:?}", k))
        .collect::<Vec<_>>().join(", "));
    let def = StructureDefinition {
        name,
        version: 1,
        fields,
    };
    let id = self.assets.insert(Box::new(def));
    self.tuple_schemas.insert(key, id);
    Ok(id)
}
```

Note `intern_tuple_schema` calls `self.compile_ty` recursively for element types, which may itself recursively call `intern_tuple_schema` for nested tuples — handled by the cache.

### Change 2 — `compile_ty` for tuples

Replace:
```rust
TyKind::Tuple(tys) => todo!("Todo tuple: {:?}", tys),
```

with:
```rust
TyKind::Tuple(tys) => {
    let struct_id = self.intern_tuple_schema(span, ty)?;
    // Build a placeholder ValueStruct with default fields; actual values are
    // set by Rvalue::Aggregate or by STRUCT_SPLIT for reads.
    ValueStruct::new(struct_id, tys.iter()
        .map(|t| self.compile_ty(span, t))
        .collect::<Result<_>>()?).into()
}
```

The `tys.iter().map(|t| self.compile_ty(span, t))` builds the placeholder field values. For nested tuples, this recurses and produces nested `ValueStruct` placeholders with the correct struct_ids. The placeholder fields are never observed because `compile_assign` writes the full struct value.

### Change 3 — `Rvalue::Aggregate(AggregateKind::Tuple, fields)`

Add an arm to `compile_assign_rvalue` in `core/src/compile/func.rs`, before the catch-all `todo!()` bucket:

```rust
Rvalue::Aggregate(box AggregateKind::Tuple, fields) => {
    let struct_id = self.compiler.intern_tuple_schema(span, ty)?;
    let mut node = NodeKind::expr(
        300002, // STRUCT_ASSEMBLY kernel id
        vec![], // dynamic inputs filled below
        ValueStruct::new(struct_id, fields.iter()
            .map(|f| self.compiler.compile_ty(span, f.ty(&self.body.local_decls, self.tcx)))
            .collect::<Result<_>>()?).into(),
    );
    // The struct_id selector must be set so the engine knows which schema to use.
    node.selectors_in[0] = struct_id.into();
    let node_ref = self.graph.insert(Node::new(node));
    for (i, field) in fields.iter().enumerate() {
        let v = self.compile_operand(field, span)?;
        // STRUCT_ASSEMBLY uses dynamic input pins at offsets matching the field count.
        self.graph.set_value_in(Connection(node_ref, i as i32), v);
    }
    ValueIn::link(Connection(node_ref, 0).into())
}
```

Wait — the actual wiring of STRUCT_ASSEMBLY's dynamic inputs depends on the engine. The existing `NODE_ASSEMBLE_STRUCT` constant is a placeholder; we'll need to verify and possibly extend it. **Risk flagged** — see Risks below.

Note: `AggregateKind` is a rustc enum with several variants (`Adt`, `Tuple`, `Closure`, `Coroutine`, etc.). This spec only handles `Tuple`. Other variants continue to panic in the catch-all bucket.

### Change 4 — Field access via Place projections

Update `compile_assign` (`core/src/compile/func.rs`) to handle `ProjectionElem::Field`:

```rust
fn compile_assign(&mut self, place: Place, value: ValueIn) -> Result<Block> {
    if !place.projection.is_empty() {
        // Walk projections to handle nested fields
        return self.compile_assign_field(place, value);
    }
    // ... existing path unchanged
}
```

And `compile_operand` similarly. The helper `compile_assign_field` walks the projection chain, inserting STRUCT_SPLIT nodes as needed. For a single `Field(i)` projection:

```rust
fn compile_assign_field(&mut self, place: Place, value: ValueIn) -> Result<Block> {
    let local_ref = *self.locals.get(place.local).unwrap();
    let base_kind = self.compiler.compile_ty(place.local.span(&self.body.local_decls), self.body.local_decls[place.local].ty)?;
    let mut current_input = ValueIn::link(Connection(local_ref, 1).into()); // local's value output
    let mut current_kind: AnyValue = base_kind.clone();
    for elem in &place.projection {
        match elem {
            ProjectionElem::Field(idx, _) => {
                let node = NodeKind::new(
                    300003, // STRUCT_SPLIT kernel id
                    0, 0,
                    vec![current_kind.clone()],
                    vec![],
                );
                let node_ref = self.graph.insert(Node::new(node));
                self.graph.set_value_in(Connection(node_ref, 0), current_input);
                current_input = ValueIn::link(Connection(node_ref, *idx).into());
                // Update current_kind to the field type
                current_kind = match current_kind.as_ref() {
                    Some(v) if let Some(s) = v.downcast_ref::<ValueStruct>() => s.fields[*idx as usize].clone(),
                    _ => return self.span_err(/* ... */),
                };
            }
            ProjectionElem::Downcast(_) | ProjectionElem::ConstantIndex { .. }
            | ProjectionElem::Subslice { .. } | ProjectionElem::Subtype(_) => {
                // enum downcast / array slicing / etc. — not in scope, panic.
                return self.span_err(/* ... */, format!("Unsupported projection: {:?}", elem));
            }
            _ => todo!(),
        }
    }
    // Final value_in goes to the value parameter of a set_local node that writes
    // back through the field-projection chain. This requires careful handling
    // for non-trivial projections; the simplest implementation writes to
    // the local's set_local and is sound only when the projection chain has
    // exactly one Field element and writes to the local's value slot.
    // ...
}
```

**Simplification:** for the first sub-project, **chains of `Field(i)` projections** are supported (e.g. `t.0.0`), since the `nested_tuple_first` demo uses this. Each `Field` in the chain inserts one STRUCT_SPLIT and threads the appropriate sub-field through. Deref, index, and subslice projections are deferred to follow-up sub-projects (they trigger `span_err`).

### Change 5 — Tuple→tuple casts

`Rvalue::Cast` is already handled by `cast_supported` logic in `compile_assign_rvalue`. For tuples, the cast path is:

```rust
let from_kind = self.compiler.compile_ty(span, from_ty)?;
let to_kind = self.compiler.compile_ty(span, *target_ty)?;
// If from_kind and to_kind are both ValueStruct with the same struct_id, no-op.
if from_kind.is_instance(&to_kind) {
    self.compile_operand(op, span)?
} else {
    self.span_err(span, format!("Unsupported cast {from_ty:?} → {target_ty:?} ({kind:?})"))?
}
```

The existing `is_instance` on `ValueStruct` already handles struct_id equality (`core/src/asset/value.rs:743`), so this works without further changes.

## Components

### Modified

- `core/src/compile/mod.rs`:
  - Add `tuple_schemas: HashMap<TupleKey, i64>` to `Compiler`
  - Initialize it in `Compiler::new`
  - Add `TupleKey` newtype + `Hash` impl
  - Add `intern_tuple_schema` method
  - Replace `TyKind::Tuple(tys) => todo!(...)` in `compile_ty` with struct-interning logic

- `core/src/compile/func.rs`:
  - Add `Rvalue::Aggregate(AggregateKind::Tuple, _)` arm in `compile_assign_rvalue`
  - Update `compile_assign` and `compile_operand` to handle `ProjectionElem::Field`

- `core/src/asset/node_graph/structure.rs`:
  - Possibly extend `StructureDefinition` API if needed (no changes expected based on the file's existing shape)

### Unchanged

- `core/proto/asset.proto` — no schema changes; `SStruct` already exists
- `core/src/asset/value.rs` — `ValueStruct` already encodes struct_id correctly
- `core/src/asset/node_graph/arithmetic.rs` — `NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` already exist as kernel id 300002 / 300003

## Data flow

### Constructing a tuple `let t = (a, b)`

1. MIR emits `Assign(_t, Aggregate(Tuple, [_a, _b]))`.
2. The new `Rvalue::Aggregate` arm fires.
3. `intern_tuple_schema` resolves (or creates and caches) the struct_id for `(T_a, T_b)`.
4. A `STRUCT_ASSEMBLY` node is inserted with kernel id 300002 and the struct_id selector.
5. Each field operand is compiled and connected to the node's dynamic input pin `i`.
6. The struct output is wired to `_t`'s `set_local` via the existing `compile_assign`.

### Reading a tuple field `let x = t.0`

1. MIR emits `Assign(_x, Use(Copy(Place { local: _t, projection: [Field(0)] })))`.
2. `compile_operand` enters the new projection-handling path.
3. The base local `_t`'s value-output pin is read.
4. A `STRUCT_SPLIT` node is inserted (kernel id 300003), connecting the struct input.
5. The split node's pin 0 (the first field output) is linked to `_x`'s `set_local`.

## Error handling

| Case | Behavior |
|---|---|
| Empty tuple `()` | Already filtered by `is_unit` in `compile_fn`. `compile_ty` is not called for empty-tuple locals. |
| Tuple with unsupported inner type | `intern_tuple_schema` propagates the inner `compile_ty` error (existing span_err behavior). |
| `Aggregate` of non-Tuple kind | Continues to `todo!()` (only `AggregateKind::Tuple` is wired). |
| Projection other than single `Field(i)` | `span_err` with `"Unsupported projection: ..."` (deferred to follow-ups). |
| Tuple cast to non-equal tuple | `span_err` (existing `cast_supported` path). |
| STRUCT_ASSEMBLY / STRUCT_SPLIT signature mismatch | Caught at the first compile failure during impl; may require widening `NODE_ASSEMBLE_STRUCT` or `NODE_SPLIT_STRUCT` constants. |

## Testing

There is no automated test harness for the backend. Verification is build-and-inspect.

Add 4 demo functions to `demo/src/lib.rs` to exercise tuple support:

```rust
#[unsafe(no_mangle)]
pub fn make_tuple(a: i32, b: f32) -> (i32, f32) {
    (a, b)
}

#[unsafe(no_mangle)]
pub fn tuple_first(t: (i32, f32)) -> i32 {
    t.0
}

#[unsafe(no_mangle)]
pub fn tuple_second(t: (i32, f32)) -> f32 {
    t.1
}

#[unsafe(no_mangle)]
pub fn nested_tuple_first(t: ((i32, f32), bool)) -> i32 {
    t.0.0
}
```

Verification:

```bash
cargo +nightly run -p build-demo
ls -la target/rust2genshin-demo.gia
cargo +nightly test -p rust2genshin
```

Expected:
- Pipeline completes successfully.
- `.gia` exists and is non-empty.
- 5/5 unit tests still pass.
- The `.gia` will contain new node-graph assets (the new `make_tuple`, `tuple_first`, etc. as composite nodes; the tuple struct definitions as dependency assets).
- No new clippy warnings introduced (run `cargo +nightly clippy --workspace --all-targets` and confirm count stays at 3 or below).

## Risks

- **`NODE_ASSEMBLE_STRUCT` dynamic-input shape.** The existing constant in `core/src/asset/node_graph/arithmetic.rs:654-657` is defined with `vec![]` inputs and one `ValueStruct` output — it's a placeholder. The actual engine kernel 300002 takes dynamic inputs whose count equals the field count of the struct schema. The implementation will need to clone and modify the constant per-arity, or extend the constant to carry the field count. **Mitigation:** if the kernel's `params` need to be sized at compile-time per call, the constant must be cloned and adjusted at the call site; verify during implementation by inspecting how `NODE_CREATE_DICTIONARY` handles dynamic arity (lines 644-651).

- **`NODE_SPLIT_STRUCT` per-field output.** The existing constant has `vec![]` outputs — the engine emits all fields at once. If only one field is needed, the unused outputs are dead pins (acceptable; the node graph optimizer or the editor handles dead pins). **Mitigation:** confirm the engine accepts STRUCT_SPLIT with all fields emitted but only some read; if not, route via a different mechanism.

- **Recursive `intern_tuple_schema`.** The function calls `compile_ty` on each element type, which may recursively call `intern_tuple_schema` for nested tuples. The cache prevents infinite recursion; verify with the nested tuple demo (`nested_tuple_first`).

- **`AnyValue` debug-string for the cache key.** Using `format!("{:?}", k)` for the key relies on `Debug` being stable across builds. Since `AnyValue` is a `Box<dyn Value>` and each `Value` impl has a fixed `Debug`, this should be stable. **Mitigation:** if two genuinely equal tuples produce different keys (unlikely), they'll generate duplicate struct defs but won't cause incorrect output.

- **`Place.projection` chaining beyond single Field.** The first-cut implementation only supports single-Field projections. Nested field access like `t.0.1` produces MIR with chained projections `[Field(0), Field(1)]`. The simple `compile_assign_field` will reject this with `span_err`. **Mitigation:** the `nested_tuple_first` demo uses `t.0.0` which is a single projection chain of length 2 — this MUST be supported in the first cut. The implementation must handle a chain of Field projections, inserting one STRUCT_SPLIT per field.

- **AggregateKind box pattern.** rustc wraps `AggregateKind` in a `Box` (visible in the `Rvalue::Aggregate(box kind, fields)` pattern). Implementation must match this correctly.

## Out-of-spec follow-ups

After this sub-project, the next natural follow-ups are:

1. **Tuple comparison** — requires a tuple-equal node or composite workaround.
2. **Field write `tup.0 = 42`** — uses STRUCT_MODIFY (kernel 300004) instead of STRUCT_SPLIT; design pending.
3. **Chained non-field projections** (deref, index, subslice) — general projection-handling infrastructure.
4. **Struct support (Tier 2)** — same machinery (struct schemas, ASSEMBLY/SPLIT/MODIFY) but driven by user-declared struct types rather than auto-generated tuple schemas.
5. **Pattern destructuring** `let (a, b) = tup;` — should work automatically after field access lands, but needs verification with demo cases.