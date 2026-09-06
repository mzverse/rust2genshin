# Tuple Locals via Flattening — Design Spec

**Date:** 2026-09-05
**Status:** Approved (brainstorming complete)
**Scope:** Enable tuple-typed MIR locals to actually work by shredding them into scalar sub-locals. After this spec, functions with tuple parameters, tuple return values, and `let t = (a, b);` patterns compile end-to-end through `compile_fn`'s local-init, `compile_assign`, and the composite-node pin generation.

## Context

The previous tuple-support sub-project (commits `61ca4f4`..`b194d96`) introduced `compile_ty` for `TyKind::Tuple`, `Rvalue::Aggregate(AggregateKind::Tuple, _)` via `STRUCT_ASSEMBLY`, and `Place.projection[Field(i)]` reads via `STRUCT_SPLIT`. That work unblocked tuple *types* but **failed to make tuple *locals* work** at runtime — Genshin's `node_local` (kernel id 58, in `core/src/asset/node_graph/query.rs:305`) only supports scalar types and panics on `SStruct`.

The workaround in commit `f8910933` skips tuple-typed locals (mapping them to `NodeRef::MAX`), and commit `bb8b041` removed the demo functions. The current state:
- `compile_ty` accepts `TyKind::Tuple` (returns `ValueStruct`).
- `Rvalue::Aggregate(Tuple, _)` is wired but produces a `STRUCT_ASSEMBLY` node whose output never reaches a useful consumer (since tuple locals are skipped).
- `Place.projection[Field(i)]` reads work for non-local struct values via `STRUCT_SPLIT`, but accessing a tuple local panics because the local's `NodeRef::MAX` is invalid.

This spec **removes the skip and replaces it with flattening**: each tuple-typed MIR Local is shredded into N consecutive scalar slots in the locals vector, where N is the tuple's arity (recursive for nested tuples). Field access becomes a flat-index lookup with no node-graph machinery needed.

## Scope

**In scope:**

1. New `locals: Vec<NodeRef>` + `local_ranges: IndexVec<Local, Range<usize>>` representation in `compile_fn`.
2. `compile_fn` flattens tuple-typed locals into sub-locals during the local-init loop.
3. Helper `local_node(place) -> NodeRef` that translates a `Place` (with projection chain) into a single `NodeRef` from the flat locals vector.
4. `compile_operand` and `compile_assign` use `local_node` to handle all `Place` lookups, replacing the current `self.locals.get(place.local)` pattern.
5. `compile_assign_rvalue`'s `Rvalue::Aggregate(Tuple, fields)` arm writes per-field to sub-locals (no more `STRUCT_ASSEMBLY` for tuple locals).
6. Composite-node pin generation in `compile_fn` exports N pins per tuple parameter/return (e.g., `t.field_0`, `t.field_1`), wiring each to its sub-local.
7. `compile_terminator`'s `Return` arm already exports the control out — no change needed.
8. The 4 tuple demo functions from the previous sub-project's Task 5 spec are re-added (they were removed because of the runtime limitation; this spec resolves it).

**Out of scope:**

- **User-declared struct locals** — only tuple locals are flattened. User structs continue to error at `compile_ty` (the existing `TyKind::Adt` arm). User structs can be a follow-up sub-project.
- **Tuple comparison `(a, b) == (c, d)`** — no tuple-equal node exists. Defer.
- **Field write `t.0 = 42` for non-top-level fields** — only top-level tuple assignment (`let t = (a, b);` where the whole tuple is replaced) is supported initially. Partial-field writes require STRUCT_MODIFY (kernel 300004) and a different design.
- **STRUCT_ASSEMBLY / STRUCT_SPLIT removal** — these node constants remain in `core/src/asset/node_graph/arithmetic.rs` and may be used by future struct support. They're no longer invoked from the tuple-local paths after this spec.
- **Special-casing nested-tuple projection** — `t.0.1` walks the projection chain with `Field(0)` then `Field(1)`, producing flat offset 1. The type info (`(i32, f32)` for `_t.0`) is implicit in the flattened indices, not tracked separately.

## Approach

### Change 1 — `compile_fn` locals representation

Replace `let mut locals = IndexVec::<Local, NodeRef>::new();` (currently around `core/src/compile/mod.rs:425`) with two parallel structures:

```rust
let mut locals: Vec<NodeRef> = Vec::new();
let mut local_ranges: IndexVec<Local, std::ops::Range<usize>> = IndexVec::new();
```

Pass `&locals` (and `&local_ranges`) into `CompilingFn` instead of just `&locals`.

### Change 2 — local-init loop (flattening)

Replace the current local-init loop:

```rust
for x in &body.local_decls {
    if is_unit(x.ty) {
        locals.push(NodeRef::from(usize::MAX));
        continue;
    }
    let kind = self.compile_ty(...)?;
    if matches!(kind.get_server_type(), ServerTypeId::SStruct) {
        locals.push(NodeRef::from(usize::MAX));
        continue;
    }
    let local = graph.insert(Node::new(node_local(kind.clone())));
    if kind.encode_storage(Side::Server).is_some() {
        graph.set_default(Connection(local, 0), kind);
    }
    locals.push(local);
}
```

with:

```rust
for x in &body.local_decls {
    let start = locals.len();
    if is_unit(x.ty) {
        locals.push(NodeRef::from(usize::MAX));
        local_ranges.push(start..locals.len());
        continue;
    }
    let kind = self.compile_ty(x.source_info.span, self.monomorphize(func, x.ty))?;
    if matches!(kind.get_server_type(), ServerTypeId::SStruct) {
        // Flatten tuple local into scalar sub-locals (one per field, recursive for nested)
        if let Some(s) = kind.downcast_ref::<ValueStruct>() {
            for field_kind in &s.fields {
                let sub = graph.insert(Node::new(node_local(field_kind.clone())));
                if field_kind.encode_storage(Side::Server).is_some() {
                    graph.set_default(Connection(sub, 0), field_kind.clone());
                }
                locals.push(sub);
            }
        }
        local_ranges.push(start..locals.len());
        continue;
    }
    // Scalar local (existing path)
    let local = graph.insert(Node::new(node_local(kind.clone())));
    if kind.encode_storage(Side::Server).is_some() {
        graph.set_default(Connection(local, 0), kind);
    }
    locals.push(local);
    local_ranges.push(start..locals.len());
}
```

### Change 3 — `local_node` helper

Add to `CompilingFn`:

```rust
/// Translate a `Place` (possibly with a Field projection chain) to a single
/// `NodeRef` from the flat locals vector. Returns the unit sentinel
/// `NodeRef::MAX` for unit locals. Errors on unsupported projections or
/// out-of-bounds field indices.
fn local_node(&self, place: Place<'tcx>, span: Span) -> Result<NodeRef> {
    let range = self.local_ranges.get(place.local).unwrap().clone();
    if range.is_empty() {
        return Ok(NodeRef::from(usize::MAX));
    }
    let mut offset = 0;
    for elem in place.projection {
        match elem {
            ProjectionElem::Field(idx, _) => {
                offset += idx.as_usize();
            }
            other => return self.span_err(
                span,
                format!("Unsupported projection element: {:?}", other),
            ),
        }
    }
    if offset >= range.len() {
        return self.span_err(
            span,
            format!(
                "Field offset {} out of bounds for tuple with {} field(s)",
                offset,
                range.len()
            ),
        );
    }
    Ok(self.locals[range.start + offset])
}
```

### Change 4 — `compile_operand` uses `local_node`

Replace the current `compile_operand` (in `core/src/compile/func.rs`, around lines 173-185):

```rust
Operand::Copy(p) |
Operand::Move(p) => {
    if !p.projection.is_empty() {
        return self.compile_operand_projection(*p, span);
    }
    ValueIn::link(Connection(*self.locals.get(p.local).unwrap(), 1).into())
}
```

with:

```rust
Operand::Copy(p) |
Operand::Move(p) => {
    let node_ref = self.local_node(*p, span)?;
    if node_ref == NodeRef::from(usize::MAX) {
        return Ok(ValueIn::value(crate::asset::value::ValueBool::def()));
    }
    ValueIn::link(Connection(node_ref, 1).into())
}
```

The unit local returns a `ValueBool::def()` (a placeholder boolean) since `node_local` doesn't apply for unit locals. This matches the existing convention (the old `compile_assign` already returns a `set_local` node with the unit local's type, which works because unit types are filtered out upstream).

`compile_operand_projection` (the previous sub-project's Task 3 helper) is removed — its job is replaced by `local_node`. STRUCT_SPLIT is no longer used for tuple-local field access.

### Change 5 — `compile_assign` keeps existing path

`compile_assign`'s behavior is unchanged for this sub-project: empty projection = scalar write (existing path), non-empty projection = span_err (deferred to a follow-up sub-project that adds `STRUCT_MODIFY` support). The whole-tuple assignment path is handled by Change 6's `Rvalue::Aggregate` arm.

### Change 6 — `Rvalue::Aggregate(Tuple, fields)` arm

Update to write per-field via `compile_assign`:

```rust
Rvalue::Aggregate(kind, fields) if matches!(**kind, AggregateKind::Tuple) => {
    // Whole-tuple aggregate: write each field to its corresponding sub-local.
    // The target local `_t` (the function's `place.local`) has been flattened
    // into N sub-locals during compile_fn's local-init; field index `i`
    // corresponds to sub-local at `local_ranges[place.local].start + i`.
    let mut combined_block = Block::nop(self.graph);
    for (field_idx, field_operand) in fields.iter().enumerate() {
        // Build a Place with the projection [Field(field_idx)] for this sub-local.
        let sub_place = Place {
            local: place.local,
            projection: self.tcx.mk_place_elems(&[ProjectionElem::Field(
                FieldIdx::from_usize(field_idx),
                /* ty */ (),
            )]),
        };
        let value = self.compile_operand(field_operand, span)?;
        let block = self.compile_assign(sub_place, value)?;
        combined_block.extend(self.graph, block);
    }
    Ok(combined_block)
}
```

(The `kind` binding is `&Box<AggregateKind<'tcx>>`, so `**kind` derefs to `AggregateKind`. The catch-all `Rvalue::Aggregate(_, _)` still panics for non-Tuple.)

### Change 7 — composite-node pin generation

Update the parameter-export and return-export loops in `compile_fn` (`core/src/compile/mod.rs` around lines 467-476):

```rust
let mut next_pin = 0;
for (i, param) in self.tcx.fn_arg_idents(func.def_id()).iter().enumerate() {
    let param_kind = self.compile_ty(param_span, self.monomorphize(func, param.ty))?;
    let base_name = param.as_ref().map(Ident::to_string).unwrap_or_else(|| format!("arg{}", i));
    if matches!(param_kind.get_server_type(), ServerTypeId::SStruct) {
        // Tuple parameter: emit N pins with dot-suffixed names
        if let Some(s) = param_kind.downcast_ref::<ValueStruct>() {
            for (field_idx, _field_kind) in s.fields.iter().enumerate() {
                let pin_name = format!("{}.field_{}", base_name, field_idx);
                graph.extra.pins.get_mut(...InValue).unwrap().push(pin_name);
                let sub = locals[*local_ranges.get(Local::arg(i)).unwrap().start + field_idx];
                graph.export_value_in(Connection(sub, 0), next_pin);
                next_pin += 1;
            }
        }
    } else {
        // Scalar parameter (existing path)
        graph.extra.pins.get_mut(...InValue).unwrap().push(base_name);
        let node = locals[*local_ranges.get(Local::arg(i)).unwrap().start];
        graph.export_value_in(Connection(node, 0), next_pin);
        next_pin += 1;
    }
}

if !is_unit(body.return_ty()) {
    let ret_kind = self.compile_ty(ret_span, body.return_ty())?;
    let base_name = "";  // or "result"
    if matches!(ret_kind.get_server_type(), ServerTypeId::SStruct) {
        // Tuple return: emit N pins
        if let Some(s) = ret_kind.downcast_ref::<ValueStruct>() {
            for (field_idx, _) in s.fields.iter().enumerate() {
                graph.extra.pins.get_mut(...OutValue).unwrap().push(format!("result.field_{}", field_idx));
                let sub = locals[*local_ranges.get(mir::RETURN_PLACE).unwrap().start + field_idx];
                graph.export_value_out(Connection(sub, 1), next_pin);
                next_pin += 1;
            }
        }
    } else {
        // Scalar return (existing path)
        graph.extra.pins.get_mut(...OutValue).unwrap().push("".into());
        let node = locals[*local_ranges.get(mir::RETURN_PLACE).unwrap().start];
        graph.export_value_out(Connection(node, 1), 0);
    }
}
```

### Change 8 — re-add the 4 tuple demo functions

In `demo/src/lib.rs`, append:

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

These previously panicked at runtime due to the now-resolved tuple-local limitation. They should now compile cleanly through the demo pipeline.

## Components

### Modified

- `core/src/compile/mod.rs`:
  - `compile_fn` local-init loop (flatten tuples).
  - Composite-node pin generation (per-field pins for tuple parameters/returns).
  - `CompilingFn` struct: add `local_ranges` field.
  - `Locals` type change in `compile_fn`.

- `core/src/compile/func.rs`:
  - `compile_operand`: use `local_node` instead of `self.locals.get(...)`.
  - `compile_assign`: per-field writes for partial-field writes; whole-tuple via Aggregate arm.
  - `Rvalue::Aggregate(Tuple, fields)` arm: per-field writes via `compile_assign`.
  - Remove `compile_operand_projection` (replaced by `local_node`).
  - Add `local_node` helper.

### Unchanged

- `core/src/asset/value.rs` — `ValueStruct` already correct.
- `core/src/asset/node_graph/arithmetic.rs` — `NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` remain in the codebase (no longer used for tuple locals; available for future struct support).
- `core/proto/asset.proto` — no schema changes.
- `core/src/asset/node_graph/structure.rs` — `StructureDefinition` machinery still in place.

## Data flow

### Constructing a tuple `let t = (a, b);`

1. MIR: `Assign(_t, Aggregate(Tuple, [_a, _b]))`.
2. `compile_fn` flattens `_t` into sub-locals at offsets 0 and 1.
3. The `Rvalue::Aggregate(Tuple, _)` arm iterates fields:
   - Emits `compile_assign` for `_t` projection `[Field(0)]` with value `_a` → writes to sub-local at offset 0.
   - Emits `compile_assign` for `_t` projection `[Field(1)]` with value `_b` → writes to sub-local at offset 1.
4. No `STRUCT_ASSEMBLY` node — the storage is per-scalar via `node_local` + `set_local`.

### Reading a tuple field `t.0`

1. MIR: `Assign(_x, Use(Copy(Place { local: _t, projection: [Field(0)] })))`.
2. `compile_operand` calls `local_node(place)` → returns `self.locals[range.start + 0]`.
3. The result is a `Connection(local_ref, 1)` linking to the sub-local's output pin.

### Function with tuple parameter and return `fn pair_first(t: (i32, f32)) -> i32`

1. The function's 2 InValue pins are named `t.field_0` (i32) and `t.field_1` (f32). Each pin exports to the corresponding sub-local of `_t`.
2. The function body accesses `t.0` via `local_node(Place { local: _t, projection: [Field(0)] })` → sub-local at offset 0.
3. The return type is `i32`, so 1 OutValue pin (no flattening needed). The return local's sub-local at offset 0 outputs to this pin.

### Nested tuple access `t.0.0` where `t: ((i32, f32), bool)`

1. `_t` flattens into 3 sub-locals: `_t_0_0` (i32), `_t_0_1` (f32), `_t_1` (bool), at offsets 0, 1, 2 respectively (depth-first pre-order).
2. `t.0.0` projection `[Field(0), Field(0)]` → `local_node` walks the projection, accumulating offset: 0+0 = 0. Returns `self.locals[range.start + 0]` = `_t_0_0`.

## Error handling

| Case | Behavior |
|---|---|
| Unit local read | Returns `ValueBool::def()` (placeholder; unit locals are filtered by `is_unit` upstream in most paths). |
| Unit local write | `span_err` "Cannot assign to unit local". |
| Empty tuple | Already filtered by `is_unit`. `compile_ty`'s empty-tuple arm has `unreachable!()`. |
| Out-of-bounds field index | `span_err` "Field offset N out of bounds for tuple with M field(s)". |
| Non-Field projection element (Deref, Index, ConstantIndex, etc.) | `span_err` "Unsupported projection element: …". |
| User-declared struct type | `compile_ty` errors at the `TyKind::Adt` arm with "Adt: …". Existing behavior. |

## Testing

There is no automated test harness for the backend. Verification is build-and-inspect:

1. The 4 demo functions above are added to `demo/src/lib.rs`.
2. Run `cargo +nightly run -p build-demo` — expect success.
3. Inspect `target/rust2genshin_demo.gia`:
   - The new composite node for `make_tuple` has 2 InValue pins (`a`, `b` — both scalar) and 2 OutValue pins (`result.field_0`, `result.field_1`).
   - The new composite node for `tuple_first` has 2 InValue pins (`t.field_0`, `t.field_1`) and 1 OutValue pin (`""` for the scalar return).
   - The new composite node for `nested_tuple_first` has 3 InValue pins (`t.field_0`, `t.field_1`, `t.field_2`) and 1 OutValue pin.
4. `cargo +nightly test -p rust2genshin` — 5/5 pass.
5. `cargo +nightly clippy --workspace --all-targets` — only the 3 environmental warnings.

## Risks

- **`Locals` type change** — every `self.locals.get(place.local)` site becomes `self.local_node(place, span)?`. Audit checklist: `core/src/compile/func.rs` has the most sites; `core/src/compile/mod.rs` has a few.
- **Field type inference for partial writes** — `tuple_field_kind` requires walking the tuple structure recursively to find the type at the projection chain's offset. If the tuple is generic or has user types, this could fail. Mitigation: defer partial-field writes (only support top-level whole-tuple assignment initially).
- **Composite-node pin generation correctness** — the per-field pin export must match the local-init order exactly. Mitigation: derive both from the same `compile_ty` call result.
- **Drop semantics** — Rust requires destructors to run when locals go out of scope. The current backend ignores drops (no `Drop` handling); flattened tuple locals would also need to be ignored. Mitigation: same as today (drop is unimplemented).
- **Test coverage of nested tuples** — the `nested_tuple_first` demo exercises `t.0.0` on a nested tuple. If the depth-first pre-order indexing is wrong, this will surface immediately.
- **Performance** — flattening a tuple local into N `node_local` nodes is O(N) node insertions per local. For typical Rust functions with shallow tuples, this is fine.

## Out-of-spec follow-ups

After this sub-project, the next natural follow-ups are:

1. **Field writes `t.0 = 42`** — uses `STRUCT_MODIFY` (kernel 300004) for partial-field writes; needs design.
2. **Tuple comparison `(a, b) == (c, d)`** — needs a tuple-equal node or composite workaround.
3. **User-declared struct locals** — extends the flattening to all `SStruct` locals, including user structs. Requires struct support.
4. **STRUCT_ASSEMBLY / STRUCT_SPLIT removal** — these node constants can be deleted from the asset layer once nothing references them. Not urgent.
5. **Pattern destructuring `let (a, b) = t;`** — should work automatically with flattening, but needs verification with a demo.