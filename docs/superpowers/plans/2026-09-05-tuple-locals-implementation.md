# Tuple Locals via Flattening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable tuple-typed MIR locals by shredding each into N consecutive scalar sub-locals. After this plan, functions with tuple parameters, tuple returns, and `let t = (a, b);` patterns compile end-to-end through `compile_fn`, `compile_assign`, `compile_operand`, and the composite-node pin generation.

**Architecture:** Seven sequential tasks. Task 1 introduces the new locals representation. Task 2 updates `compile_fn`'s local-init loop to flatten tuples. Task 3 introduces `local_node` and rewires `compile_operand`. Task 4 updates composite-node pin generation. Task 5 rewrites the `Aggregate(Tuple, _)` arm. Task 6 removes the now-obsolete `compile_operand_projection` and the tuple-local skip. Task 7 re-adds the demo functions and verifies the pipeline.

**Tech Stack:** Rust nightly, `rustc_private` API (`rustc_middle::mir::{Place, ProjectionElem, AggregateKind, FieldIdx, Operand, Local, mir}`, `rustc_index::IndexVec`), existing `crate::asset::{node_local, node_set_local, Node, Connection, ValueIn, Block}`.

**Project root:** `F:/rust2genshin/`

---

## File Structure

**Modified:**
- `core/src/compile/mod.rs` — `compile_fn`: locals representation + flatten loop + composite pin generation.
- `core/src/compile/func.rs` — `compile_operand`, `compile_assign`, `Rvalue::Aggregate` arm, add `local_node`, remove `compile_operand_projection`.
- `core/src/compile/mod.rs` — `CompilingFn` struct: add `local_ranges` field.
- `demo/src/lib.rs` — re-add 4 tuple demo functions.

**Unchanged:**
- `core/src/asset/value.rs` — `ValueStruct` already correct.
- `core/src/asset/node_graph/arithmetic.rs` — `NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` remain (no longer used by tuple locals; available for future struct support).
- `core/proto/asset.proto` — no changes.

**No new files.**

---

## Task 1: Replace `Locals` representation with `Vec<NodeRef>` + `Range` map

**Files:**
- Modify: `core/src/compile/mod.rs` — `compile_fn` locals initialization + `CompilingFn` struct field.

- [ ] **Step 1: Update the `CompilingFn` struct definition**

Open `core/src/compile/mod.rs`. Find the `CompilingFn` struct (around lines 131-137):

```rust
pub(crate) struct CompilingFn<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraph<NodeGraphComposite>,
    pub body: &'a Body<'tcx>,
    pub locals: &'a IndexVec<Local, NodeRef>,
}
```

Replace with:

```rust
pub(crate) struct CompilingFn<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraph<NodeGraphComposite>,
    pub body: &'a Body<'tcx>,
    pub locals: &'a Vec<NodeRef>,
    pub local_ranges: &'a IndexVec<Local, std::ops::Range<usize>>,
}
```

(Add the `use std::ops::Range` if not already imported, or use the fully-qualified path as shown.)

- [ ] **Step 2: Update `compile_fn` locals initialization**

Open `core/src/compile/mod.rs`. Find `compile_fn` (around line 419) and the locals initialization. Look for:

```rust
let mut locals = IndexVec::<Local, NodeRef>::new(); // TODO: adapt for struct, struct list and map
```

Replace with:

```rust
let mut locals: Vec<NodeRef> = Vec::new();
let mut local_ranges: IndexVec<Local, std::ops::Range<usize>> = IndexVec::new();
```

The local-init loop will be updated in Task 2 to populate these correctly.

- [ ] **Step 3: Update the `CompilingFn` instantiation in `compile_fn`**

Find the block where `CompilingFn` is constructed:

```rust
let mut compiling = CompilingFn {
    tcx: self.tcx,
    compiler: self,
    graph: &mut graph,
    body,
    locals: &locals,
};
```

Replace with:

```rust
let mut compiling = CompilingFn {
    tcx: self.tcx,
    compiler: self,
    graph: &mut graph,
    body,
    locals: &locals,
    local_ranges: &local_ranges,
};
```

- [ ] **Step 4: Find all current `self.locals.get(place.local)` and `*self.locals.get(...)` sites**

Run:
```bash
cd F:/rust2genshin && grep -rn "self.locals.get\|\*self.locals" core/src/compile/
```

Expected: lists every site that reads the old `IndexVec<Local, NodeRef>`. We'll update these in Task 3.

Note: the local-init loop still has `locals.push(...)` calls. For now, this is broken (the loop doesn't push anything yet because the loop body changes in Task 2). To keep the build green, we'll add a placeholder push in Task 2.

- [ ] **Step 5: Build to verify (expect failure)**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build errors (the old `IndexVec` no longer has `get(p.local)` semantics; the `CompilingFn.locals` type changed; sites haven't been updated yet).

- [ ] **Step 6: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "refactor(core): change CompilingFn locals to Vec<NodeRef> + Range map"
```

---

## Task 2: Update `compile_fn` local-init loop to flatten tuples

**Files:**
- Modify: `core/src/compile/mod.rs` — local-init loop.

- [ ] **Step 1: Replace the local-init loop**

Open `core/src/compile/mod.rs`. Find the local-init loop (currently around lines 425-437 after Task 1's struct change). The loop currently is:

```rust
let mut locals: Vec<NodeRef> = Vec::new();
let mut local_ranges: IndexVec<Local, std::ops::Range<usize>> = IndexVec::new();
for x in &body.local_decls {
    let start = locals.len();
    // [PLACEHOLDER: loop body to be added in step 2]
    local_ranges.push(start..locals.len());
}
```

Wait — the current loop body from before Task 1's change looked like:

```rust
for x in &body.local_decls {
    if is_unit(x.ty) {
        locals.push(NodeRef::from(usize::MAX));
        continue;
    }
    let kind = self.compile_ty(x.source_info.span, self.monomorphize(func, x.ty))?;
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

After Task 1's change, this loop needs the `local_ranges.push(...)` line and the flatten logic. Replace the loop with:

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

- [ ] **Step 2: Add `ValueStruct` import if missing**

In `core/src/compile/mod.rs`, the import block (line 5) should already include `ValueStruct` from Task 2 of the previous tuple-support sub-project. Verify:

```bash
cd F:/rust2genshin && grep "ValueStruct" core/src/compile/mod.rs
```

If not present, add to the import list on line 5.

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: still build errors (the `compile_operand` and other sites still use `self.locals.get(place.local)` which is now invalid on `Vec<NodeRef>`). Task 3 fixes those.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "feat(core): flatten tuple locals into scalar sub-locals in compile_fn"
```

---

## Task 3: Add `local_node` helper and update `compile_operand`

**Files:**
- Modify: `core/src/compile/func.rs` — add `local_node` helper, update `compile_operand` to use it, update any other `self.locals.get(place.local)` sites.

- [ ] **Step 1: Add the `local_node` helper to `CompilingFn`**

Open `core/src/compile/func.rs`. Add the following method inside `impl<'tcx, 'a> CompilingFn<'tcx, 'a>` (near the existing `compile_operand`):

```rust
/// Translate a `Place` (possibly with a Field projection chain) to a single
/// `NodeRef` from the flat locals vector. Returns the unit sentinel
/// `NodeRef::MAX` for unit locals. Errors on unsupported projections or
/// out-of-bounds field indices.
fn local_node(&self, place: Place<'tcx>, span: Span) -> Result<NodeRef> {
    use rustc_middle::mir::ProjectionElem;
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

- [ ] **Step 2: Update `compile_operand` to use `local_node`**

Find the current `compile_operand` (around lines 173-185):

```rust
fn compile_operand(&mut self, op: &Operand<'tcx>, span: Span) -> Result<ValueIn> {
    Ok(match op {
        Operand::Copy(p) |
        Operand::Move(p) => {
            if !p.projection.is_empty() {
                return self.compile_operand_projection(*p, span);
            }
            ValueIn::link(Connection(*self.locals.get(p.local).unwrap(), 1).into())
        }
        // ... other arms ...
    })
}
```

Replace the `Operand::Copy | Operand::Move` arm with:

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

The unit case returns a `ValueBool::def()` placeholder (matches the existing convention for unit types).

- [ ] **Step 3: Find any other `self.locals.get(place.local)` sites**

Run:
```bash
cd F:/rust2genshin && grep -n "self.locals.get\|\*self.locals" core/src/compile/func.rs
```

Expected: zero matches after Task 1 (the loop body now pushes `NodeRef`s but doesn't use `self.locals.get`).

If any remain, update them to use `self.local_node(place, span)?` instead.

- [ ] **Step 4: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds (or only one or two remaining compile errors in `compile_assign` and other sites that we'll fix in later tasks).

If build errors remain in `compile_assign` (line 59: `*self.locals.get(place.local).unwrap()`), update:

```rust
self.graph.connect_value(Connection(*self.locals.get(place.local).unwrap(), 0), Connection(node, 0));
```

to:

```rust
let local_ref = self.local_node(place, decl.source_info.span)?;
if local_ref == NodeRef::from(usize::MAX) {
    return Ok(Block::nop(self.graph));
}
self.graph.connect_value(Connection(local_ref, 0), Connection(node, 0));
```

(This handles the unit case where `local_node` returns `NodeRef::MAX`.)

- [ ] **Step 5: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 6: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): add local_node helper, update compile_operand to use flat locals"
```

---

## Task 4: Update composite-node pin generation for tuple parameters and returns

**Files:**
- Modify: `core/src/compile/mod.rs` — parameter and return pin generation loops in `compile_fn`.

- [ ] **Step 1: Locate the parameter and return pin generation**

Open `core/src/compile/mod.rs`. Find the section after the local-init loop and `CompilingFn` block (around lines 467-476):

```rust
graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InControl).unwrap().push("".into());
graph.export_control_in(blocks.get(mir::START_BLOCK).unwrap().begin, 0);
graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutControl).unwrap().push("".into());
for (i, param) in self.tcx.fn_arg_idents(func.def_id()).iter().enumerate() {
    graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(param.as_ref().map(Ident::to_string).unwrap_or_else(|| format!("arg{}", i).to_string()));
    let node = *locals.get(Local::arg(i)).unwrap();
    graph.export_value_in(Connection(node, 0), i);
}
if !is_unit(body.return_ty()) {
    graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push("".into());
    let node = *locals.get(mir::RETURN_PLACE).unwrap();
    graph.export_value_out(Connection(node, 1), 0);
}
```

- [ ] **Step 2: Replace with the flattening-aware version**

Replace the entire block (after the OutControl pin push) with:

```rust
graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InControl).unwrap().push("".into());
graph.export_control_in(blocks.get(mir::START_BLOCK).unwrap().begin, 0);
graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutControl).unwrap().push("".into());

let mut next_in_pin = 0;
for (i, param) in self.tcx.fn_arg_idents(func.def_id()).iter().enumerate() {
    let param_kind = self.compile_ty(/* param span */ self.body.span, self.monomorphize(func, /* param ty */ /* ... */))?;
    let base_name = param.as_ref().map(Ident::to_string).unwrap_or_else(|| format!("arg{}", i));
    let range = local_ranges.get(Local::arg(i)).unwrap().clone();
    if matches!(param_kind.get_server_type(), ServerTypeId::SStruct) {
        // Tuple parameter: emit N pins with dot-suffixed names
        if let Some(s) = param_kind.downcast_ref::<ValueStruct>() {
            for (field_idx, _field_kind) in s.fields.iter().enumerate() {
                let pin_name = format!("{}.field_{}", base_name, field_idx);
                graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(pin_name);
                let sub = locals[range.start + field_idx];
                graph.export_value_in(Connection(sub, 0), next_in_pin);
                next_in_pin += 1;
            }
        }
    } else {
        // Scalar parameter (existing path)
        graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(base_name);
        let node = locals[range.start];
        graph.export_value_in(Connection(node, 0), next_in_pin);
        next_in_pin += 1;
    }
}

let mut next_out_pin = 0;
if !is_unit(body.return_ty()) {
    let ret_kind = self.compile_ty(/* return span */ /* ... */, body.return_ty())?;
    let ret_range = local_ranges.get(mir::RETURN_PLACE).unwrap().clone();
    if matches!(ret_kind.get_server_type(), ServerTypeId::SStruct) {
        // Tuple return: emit N pins
        if let Some(s) = ret_kind.downcast_ref::<ValueStruct>() {
            for (field_idx, _field_kind) in s.fields.iter().enumerate() {
                graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push(format!("result.field_{}", field_idx));
                let sub = locals[ret_range.start + field_idx];
                graph.export_value_out(Connection(sub, 1), next_out_pin);
                next_out_pin += 1;
            }
        }
    } else {
        // Scalar return (existing path)
        graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push("".into());
        let node = locals[ret_range.start];
        graph.export_value_out(Connection(node, 1), next_out_pin);
        next_out_pin += 1;
    }
}
```

**Note:** the spec might need adjustments based on the actual rustc API for fetching param types and spans. Investigate `fn_arg_idents` and `body.local_decls[Local::arg(i)].ty` to find the right way to get the MIR-level parameter type.

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds. If there are API mismatches, search the codebase for similar patterns (`monomorphize`, `compile_ty` with `body.local_decls`).

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "feat(core): flatten tuple parameters/returns into per-field composite pins"
```

---

## Task 5: Update `Rvalue::Aggregate(Tuple, fields)` arm to write per-field

**Files:**
- Modify: `core/src/compile/func.rs` — `Rvalue::Aggregate(AggregateKind::Tuple, _)` arm.

- [ ] **Step 1: Locate the Aggregate arm**

Open `core/src/compile/func.rs`. Find the current `Rvalue::Aggregate(kind, fields) if matches!(**kind, AggregateKind::Tuple) => { ... }` arm (added in Task 4 of the previous tuple-support sub-project).

The current body uses STRUCT_ASSEMBLY. Replace it with the per-field writes version.

- [ ] **Step 2: Replace with per-field writes via `compile_assign`**

Replace the entire Aggregate arm body with:

```rust
{
    // Whole-tuple aggregate: write each field to its corresponding sub-local.
    // The target local `_t` (the function's `place.local`) has been flattened
    // into N sub-locals during compile_fn's local-init; field index `i`
    // corresponds to sub-local at `local_ranges[place.local].start + i`.
    let mut combined_block = Block::nop(self.graph);
    for (field_idx, field_operand) in fields.iter().enumerate() {
        let sub_place = Place {
            local: place.local,
            projection: self.tcx.mk_place_elems(&[ProjectionElem::Field(
                rustc_middle::mir::FieldIdx::from_usize(field_idx),
                /* ty */ (),
            )]),
        };
        let value = self.compile_operand(field_operand, span)?;
        let block = self.compile_assign(sub_place, value)?;
        combined_block.extend(self.graph, block);
    }
    return Ok(combined_block);
}
```

**Note:** the `place.local` access requires that `place` is available in scope (it's a parameter of `compile_assign_rvalue`). `ProjectionElem::Field` may need different syntax — verify against the actual rustc API.

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

- [ ] **Step 4: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): rewrite Aggregate(Tuple, _) arm to write per-field via compile_assign"
```

---

## Task 6: Remove `compile_operand_projection` and the tuple-local skip

**Files:**
- Modify: `core/src/compile/func.rs` — remove obsolete helpers.

- [ ] **Step 1: Remove `compile_operand_projection`**

Open `core/src/compile/func.rs`. Find the `compile_operand_projection` method (added in Task 3 of the previous tuple-support sub-project — should have ~50 lines). Delete the entire method.

- [ ] **Step 2: Verify the tuple-local skip is gone**

In `core/src/compile/mod.rs`, the previous sub-project added:

```rust
if matches!(kind.get_server_type(), ServerTypeId::SStruct) {
    locals.push(NodeRef::from(usize::MAX));
    continue;
}
```

This was replaced by the flattening logic in Task 2. Verify the file no longer has this exact pattern (just the flatten branch).

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

- [ ] **Step 4: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs core/src/compile/mod.rs && git commit -m "refactor(core): remove obsolete compile_operand_projection and tuple-local skip"
```

---

## Task 7: Re-add the 4 tuple demo functions and verify the demo pipeline

**Files:**
- Modify: `demo/src/lib.rs` — append 4 tuple demo functions.

- [ ] **Step 1: Read current end of demo file**

Open `demo/src/lib.rs`. The file should end with `cast_i32_to_bool` (the last cast function).

- [ ] **Step 2: Append the 4 tuple demo functions**

Add at the end of `demo/src/lib.rs`:

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

- [ ] **Step 3: Run the demo pipeline**

Run:
```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: pipeline completes; `target/rust2genshin_demo.gia` exists. The `.gia` SHA will change from the previous baseline (because new composite nodes are added).

If the pipeline fails, the most likely cause is an incorrect flatten / pin-export wiring. Investigate by:
- Running `cargo +nightly build -p rust2genshin` to see compile errors first.
- Adding `eprintln!` debugging to the Aggregate arm or the pin-export loop.

- [ ] **Step 4: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add demo/src/lib.rs && git commit -m "test(demo): re-add tuple exercise functions (now supported via flattening)"
```

- [ ] **Step 6: Update the design spec to reflect the resolution**

Open `docs/superpowers/specs/2026-09-05-tuple-support-design.md`. The "Known limitation: tuple locals" section is now stale — tuple locals DO work. Replace the section with:

```markdown
## Resolved: tuple locals (via flattening)

The Genshin `node_local` kernel id 58 still only supports scalar types —
this is a fundamental engine constraint. Rather than adding new node types or,
restructuring Rust-side tuple handling, this sub-project resolved the tuple-local
limitation by **flattening**: each tuple-typed MIR Local is shredded into N
consecutive scalar slots in the locals vector, where N is the tuple's arity
(recursive for nested tuples). Field access becomes a flat-index lookup with
no node-graph machinery needed.

See `docs/superpowers/specs/2026-09-05-tuple-locals-design.md` for the full
design.
```

The new spec lives at `docs/superpowers/specs/2026-09-05-tuple-locals-design.md` and `docs/superpowers/plans/2026-09-05-tuple-locals-implementation.md`.

- [ ] **Step 7: Commit the spec update**

```bash
cd F:/rust2genshin && git add docs/superpowers/specs/2026-09-05-tuple-support-design.md && git commit -m "docs: mark 'tuple locals' limitation as resolved via flattening"
```

---

## Final verification

After all 7 tasks complete:

- [ ] **Build pipeline produces `.gia`**

```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: success; `target/rust2genshin_demo.gia` exists. The SHA will differ from any pre-flattening baseline (because new composite nodes are added for the 4 demo functions).

- [ ] **Unit tests pass**

```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Clippy is clean**

```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep "^warning:" | wc -l
```

Expected: 3 (only environmental warnings).

- [ ] **`// TODO` markers preserved**

```bash
cd F:/rust2genshin && grep -rn "// TODO" core/src lib/src demo/src | wc -l
```

Expected: 18 or higher (no intentional removal).

If any step fails, report the failure and the relevant commit to investigate.