# Tuple Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Rust tuple type support in the MIR→node-graph backend. Tuples map onto genshin `SStruct` with auto-generated struct schemas. After this plan, `compile_ty` accepts non-empty tuple types, `Rvalue::Aggregate(AggregateKind::Tuple, _)` constructs tuples, and `Place.projection[Field(i)...]` reads tuple fields.

**Architecture:** Five sequential tasks. Tasks 1-2 add the foundational cache and `compile_ty` arm. Task 3 wires field-projection reads/writes. Task 4 wires tuple construction. Task 5 adds demo functions that exercise all paths. Task 6 verifies the pipeline end-to-end.

**Tech Stack:** Rust nightly, `rustc_private` (`rustc_middle::mir::ProjectionElem`, `AggregateKind`, `Operand`, `Place`), `crate::asset::node_graph::arithmetic::{NODE_ASSEMBLE_STRUCT, NODE_SPLIT_STRUCT}`, `crate::asset::value::ValueStruct`, `crate::asset::node_graph::structure::{StructureDefinition, StructField}`. No proto changes — `SStruct` already exists in the schema.

**Project root:** `F:/rust2genshin/`

---

## File Structure

**Modified:**
- `core/src/compile/mod.rs` — add `tuple_schemas: HashMap<TupleKey, i64>` to `Compiler`; add `TupleKey` newtype; add `intern_tuple_schema` method; replace the `TyKind::Tuple(tys) => todo!()` arm in `compile_ty`.
- `core/src/compile/func.rs` — add `Rvalue::Aggregate(AggregateKind::Tuple, _)` arm; update `compile_assign` and `compile_operand` to handle `ProjectionElem::Field` chains.
- `demo/src/lib.rs` — add 4 demo functions exercising tuple construction, field access, and nested tuples.

**Unchanged:**
- `core/proto/asset.proto` — `SStruct` already exists.
- `core/src/asset/value.rs` — `ValueStruct` already encodes struct_id correctly.
- `core/src/asset/node_graph/structure.rs` — `StructureDefinition` / `StructField` already exist.

**No new files.**

---

## Task 1: Compiler cache + `intern_tuple_schema` helper

**Files:**
- Modify: `core/src/compile/mod.rs` — add `TupleKey`, `tuple_schemas` field, `intern_tuple_schema` method.

- [ ] **Step 1: Add `TupleKey` newtype and import it**

Open `core/src/compile/mod.rs`. Near the top of the file (in the import block, around line 1-30), make sure the following imports are present (add if not):

```rust
use std::collections::HashMap;
```

`HashMap` is likely already imported. If so, skip this step.

Then add a newtype after the existing types (e.g., after `pub(crate) fn is_unit` near line 144-153):

```rust
/// Cache key for interned tuple struct schemas. Uses the arity and a stable
/// debug-string of the element types' `AnyValue`s. Two equal tuples produce
/// the same key; the cache prevents duplicate struct-definition generation.
#[derive(Clone, Eq, PartialEq, Hash)]
pub(crate) struct TupleKey(pub(crate) String);

impl core::fmt::Debug for TupleKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}
```

- [ ] **Step 2: Add `tuple_schemas` field to `Compiler`**

In the `Compiler` struct definition (around line 232-238 of `core/src/compile/mod.rs`), add a new field:

```rust
pub(crate) struct Compiler<'tcx> {
    tcx: TyCtxt<'tcx>,
    lib: CrateNum,
    assets: AssetBundle,
    compiling: HashSet<Instance<'tcx>>,
    compiled: HashMap<Instance<'tcx>, i64>,
    tuple_schemas: HashMap<TupleKey, i64>,
}
```

- [ ] **Step 3: Initialize `tuple_schemas` in `Compiler::new`**

In `Compiler::new` (around line 246-263), add initialization:

```rust
Ok(Self {
    tcx, lib,
    assets: AssetBundle::new(crate::asset::GameMode::Overlimit),
    compiling: HashSet::new(),
    compiled: HashMap::new(),
    tuple_schemas: HashMap::new(),
})
```

- [ ] **Step 4: Add `intern_tuple_schema` method to `Compiler`**

Add the method inside `impl<'tcx> Compiler<'tcx>` (anywhere in the `impl Compiler` block). Insert after `compile_ty` or `find_lib_fn`:

```rust
/// Intern a tuple type as a genshin `SStruct` schema. Generates a
/// `StructureDefinition` asset on first encounter and caches the
/// resulting `struct_id` for subsequent lookups.
///
/// The cache key is a string derived from the element types' `AnyValue`
/// debug representation, plus the arity. Element types are resolved
/// recursively (so nested tuples are supported and cached too).
pub(crate) fn intern_tuple_schema(&mut self, span: Span, ty: Ty<'tcx>) -> Result<i64> {
    let TyKind::Tuple(elem_tys) = ty.kind() else {
        return self.span_err(span, "intern_tuple_schema called with non-tuple type");
    };
    // Resolve element types first (recursively interns nested tuples).
    let elem_kinds: Vec<AnyValue> = elem_tys.iter()
        .map(|t| self.compile_ty(span, t))
        .collect::<Result<_>>()?;
    // Use a string key to keep this HashMap independent of MIR type identity.
    let key = TupleKey(format!("[{}]", elem_kinds.iter()
        .map(|k| format!("{:?}", k))
        .collect::<Vec<_>>().join(", ")));
    if let Some(&id) = self.tuple_schemas.get(&key) {
        return Ok(id);
    }
    // Build the StructureDefinition.
    use crate::asset::node_graph::structure::{StructField, StructureDefinition};
    let fields: Vec<StructField> = elem_kinds.iter().enumerate()
        .map(|(i, k)| StructField {
            name: format!("field_{i}"),
            value: k.clone(),
            is_set: false,
        })
        .collect();
    let name = format!("Tuple_{}", elem_kinds.iter()
        .map(|k| format!("{:?}", k))
        .collect::<Vec<_>>().join("_"));
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

- [ ] **Step 5: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds.

If you see "cannot find type `AnyValue` in this scope" — the import for `AnyValue` is missing in `compile/mod.rs`. Add `use crate::asset::value::AnyValue;` near the top of the file.

If you see "no method named `compile_ty` found" — `compile_ty` is defined later in the file. Move the `intern_tuple_schema` method definition to **after** the `compile_ty` definition (Rust doesn't care about method order, but the impl block methods can reference each other freely; the issue is usually a missing import or wrong method signature).

- [ ] **Step 6: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "feat(core): add tuple struct_id cache and intern_tuple_schema helper"
```

---

## Task 2: `compile_ty` for `TyKind::Tuple`

**Files:**
- Modify: `core/src/compile/mod.rs` — replace the `TyKind::Tuple(tys) => todo!(...)` arm in `compile_ty`.

- [ ] **Step 1: Locate the line**

Open `core/src/compile/mod.rs`. Around line 210, find:

```rust
TyKind::Tuple(tys) => todo!("Todo tuple: {:?}", tys),
```

- [ ] **Step 2: Replace with the struct-interning logic**

Replace that single line with:

```rust
TyKind::Tuple(tys) => {
    // Empty tuples are filtered by `is_unit` before reaching here; if we
    // see one, treat it as unit. Non-empty tuples are interned as
    // genshin `SStruct` schemas via `intern_tuple_schema`.
    if tys.is_empty() {
        return Ok(crate::asset::value::ValueBool::def());
    }
    let struct_id = self.intern_tuple_schema(span, ty)?;
    ValueStruct::new(struct_id, tys.iter()
        .map(|t| self.compile_ty(span, t))
        .collect::<Result<_>>()?).into()
}
```

(Adjust to import `ValueBool` and `ValueStruct` at the top of the file if not already imported.)

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds. `compile_ty` for tuple types now returns `ValueStruct` instead of panicking.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/mod.rs && git commit -m "feat(core): compile TyKind::Tuple as ValueStruct with interned struct_id"
```

---

## Task 3: Field-projection handling in `compile_assign` and `compile_operand`

**Files:**
- Modify: `core/src/compile/func.rs` — extend `compile_assign` and `compile_operand` to handle `ProjectionElem::Field` chains.

- [ ] **Step 1: Read the current implementations**

Open `core/src/compile/func.rs`. Note the current `compile_assign` (around line 54-63) and `compile_operand` (around line 173-181). Both reject non-empty projections with `todo!()`.

- [ ] **Step 2: Add helper `compile_operand_projection` (read path)**

Add the following method to `impl<'tcx, 'a> CompilingFn<'tcx, 'a>`. Place it near `compile_operand`:

```rust
/// Resolve a Place with a non-empty projection chain to a `ValueIn`,
/// inserting `STRUCT_SPLIT` nodes as needed.
///
/// Only chains consisting entirely of `ProjectionElem::Field(i, _)` are
/// supported. Other projection kinds (Deref, Index, ConstantIndex, etc.)
/// trigger `span_err` and defer to follow-up sub-projects.
fn compile_operand_projection(&mut self, place: Place<'tcx>, span: Span) -> Result<ValueIn> {
    use crate::asset::node_graph::arithmetic::NODE_SPLIT_STRUCT;
    use rustc_middle::mir::ProjectionElem;
    let local_ref = *self.locals.get(place.local).unwrap();
    let base_kind = self.compiler.compile_ty(
        span,
        self.body.local_decls[place.local].ty,
    )?;
    // The base local's output pin is pin 1 (per node_local's contract).
    let mut current_input = ValueIn::link(Connection(local_ref, 1).into());
    let mut current_kind: AnyValue = base_kind;
    for elem in &place.projection {
        match elem {
            ProjectionElem::Field(field_idx, _) => {
                // Build a fresh STRUCT_SPLIT node per field access.
                let node = Node::new(NODE_SPLIT_STRUCT.clone());
                let node_ref = self.graph.insert(node);
                self.graph.set_value_in(Connection(node_ref, 0), current_input);
                current_input = ValueIn::link(Connection(node_ref, *field_idx).into());
                // Advance the type tracker: current_kind must be a ValueStruct
                // with a `fields` Vec; pull the field at `field_idx`.
                current_kind = match current_kind.as_ref() {
                    Some(v) if let Some(s) = v.downcast_ref::<ValueStruct>() => {
                        s.fields[*field_idx as usize].clone()
                    }
                    _ => return self.span_err(
                        span,
                        format!("Field access into non-struct: {:?}", current_kind),
                    ),
                };
            }
            other => return self.span_err(
                span,
                format!("Unsupported projection element in field access: {:?}", other),
            ),
        }
    }
    Ok(current_input)
}
```

- [ ] **Step 3: Update `compile_operand` to dispatch on projection**

Replace:
```rust
Operand::Copy(p) |
Operand::Move(p) => {
    if !p.projection.is_empty() {
        todo!("struct is still unsupported")
    }
    ValueIn::link(Connection(*self.locals.get(p.local).unwrap(), 1).into())
}
```

with:
```rust
Operand::Copy(p) |
Operand::Move(p) => {
    if !p.projection.is_empty() {
        return self.compile_operand_projection(*p, span);
    }
    ValueIn::link(Connection(*self.locals.get(p.local).unwrap(), 1).into())
}
```

- [ ] **Step 4: Update `compile_assign` to dispatch on projection**

Replace:
```rust
fn compile_assign(&mut self, place: Place, value: ValueIn) -> Result<Block> {
    if !place.projection.is_empty() {
        todo!()
    }
    let decl = self.body.local_decls.get(place.local).unwrap();
    let node = self.graph.insert(Node::new(node_set_local(self.compiler.compile_ty(decl.source_info.span, decl.ty)?)));
    self.graph.connect_value(Connection(*self.locals.get(place.local).unwrap(), 0), Connection(node, 0));
    self.graph.set_value_in(Connection(node, 1), value);
    Ok(Block::singleton(node, 0))
}
```

with:
```rust
fn compile_assign(&mut self, place: Place, value: ValueIn) -> Result<Block> {
    if !place.projection.is_empty() {
        // For write paths, we don't currently support tuple field writes
        // (would require STRUCT_MODIFY rather than STRUCT_SPLIT). Defer.
        return self.span_err(
            place.local.span(&self.body.local_decls),
            format!("Write into place projection is unsupported: {:?}", place),
        );
    }
    let decl = self.body.local_decls.get(place.local).unwrap();
    let node = self.graph.insert(Node::new(node_set_local(self.compiler.compile_ty(decl.source_info.span, decl.ty)?)));
    self.graph.connect_value(Connection(*self.locals.get(place.local).unwrap(), 0), Connection(node, 0));
    self.graph.set_value_in(Connection(node, 1), value);
    Ok(Block::singleton(node, 0))
}
```

The write path now produces a clear `span_err` instead of `todo!()` panicking.

- [ ] **Step 5: Add `AnyValue` import if needed**

In `core/src/compile/func.rs`, ensure the import block (lines 1-16) includes:

```rust
use crate::asset::value::AnyValue;
```

Add this import if not present.

- [ ] **Step 6: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds. (No demo functions yet — these are read-path wiring only; not exercised until Task 5.)

- [ ] **Step 7: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): support Place.projection[Field(i)] reads in compile_operand"
```

---

## Task 4: `Rvalue::Aggregate(AggregateKind::Tuple, fields)`

**Files:**
- Modify: `core/src/compile/func.rs` — add the Aggregate arm.

- [ ] **Step 1: Add imports**

In `core/src/compile/func.rs`, add to the import block:

```rust
use rustc_middle::mir::AggregateKind;
```

(Other rustc imports may already pull in `AggregateKind`; if `use rustc_middle::mir::{...}` is on a single line, add `AggregateKind` to that list.)

- [ ] **Step 2: Add the Aggregate arm**

In `compile_assign_rvalue` (around line 65-169), find the catch-all `todo!()` bucket near the end:

```rust
Rvalue::Repeat(_, _)
| Rvalue::ThreadLocalRef(_)
| Rvalue::Discriminant(_)
| Rvalue::Aggregate(_, _)
| Rvalue::CopyForDeref(_)
| Rvalue::WrapUnsafeBinder(_, _)
    => todo!("{:?}", value),
```

Replace with:

```rust
Rvalue::Aggregate(box AggregateKind::Tuple, fields) => {
    let struct_id = self.compiler.intern_tuple_schema(span, ty)?;
    // Build a placeholder ValueStruct for the output type so STRUCT_ASSEMBLY
    // can wire its return pin correctly.
    let placeholder = ValueStruct::new(struct_id, fields.iter()
        .map(|f| self.compiler.compile_ty(span, f.ty(&self.body.local_decls, self.tcx)))
        .collect::<Result<_>>()?);
    // Clone the static STRUCT_ASSEMBLY constant and set the struct_id selector.
    let mut node_kind = crate::asset::node_graph::arithmetic::NODE_ASSEMBLE_STRUCT.clone();
    // Override the output type to use our struct_id.
    node_kind = node_kind.with_output(placeholder.into());
    let node_ref = self.graph.insert(Node::new(node_kind));
    // Set the struct_id selector (input pin 0 is the "select which struct" pin).
    self.graph.set_value_in(
        Connection(node_ref, -1), // selectortype selector (index 0 of selectors_in)
        ValueIn::value(ValueInt(struct_id as i32).into()),
    );
    // Wire the field operands to dynamic input pins starting at index 1
    // (input 0 is reserved for the struct_id selector).
    for (i, field) in fields.iter().enumerate() {
        let v = self.compile_operand(field, span)?;
        self.graph.set_value_in(Connection(node_ref, (i + 1) as i32), v);
    }
    ValueIn::link(Connection(node_ref, 0).into())
}
Rvalue::Repeat(_, _)
| Rvalue::ThreadLocalRef(_)
| Rvalue::Discriminant(_)
| Rvalue::Aggregate(_, _) // non-Tuple AggregateKind still panics
| Rvalue::CopyForDeref(_)
| Rvalue::WrapUnsafeBinder(_, _)
    => todo!("{:?}", value),
```

**Note:** the exact wiring of STRUCT_ASSEMBLY's struct_id selector and dynamic field inputs depends on the actual signature of `NODE_ASSEMBLE_STRUCT`. If the build fails because `with_output` doesn't exist on `NodeKind` or because pin indices are wrong, inspect `core/src/asset/node_graph/arithmetic.rs:654-657` and `core/src/asset/value.rs` for `NodeKind`'s structure, and adjust accordingly. The pattern to follow is `NODE_CREATE_DICTIONARY` at `arithmetic.rs:644-651` which also takes dynamic inputs.

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds (though the code path is not yet exercised by any demo function).

If the build fails with a type mismatch on `NodeKind::with_output` or `set_value_in`, investigate the actual `NodeKind` API and adjust. The previous cast/inline-polish work has established patterns for dynamic-input nodes — search the codebase for `selectors_in` or `params` usage to find similar examples.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): translate Rvalue::Aggregate(Tuple, _) via STRUCT_ASSEMBLY"
```

---

## Task 5: Add demo functions exercising tuples

**Files:**
- Modify: `demo/src/lib.rs` — append 4 demo functions.

- [ ] **Step 1: Read the current end of the demo file**

Open `demo/src/lib.rs`. Note the current contents (the cast functions end at the file's last line `cast_i32_to_bool`).

- [ ] **Step 2: Append tuple demo functions**

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

Expected: pipeline completes; `target/rust2genshin-demo.gia` exists.

If the pipeline fails with `cast is unimplemented` or similar (regression in a previous feature), revert Task 5's demo additions, run `cargo +nightly build -p rust2genshin` to see the specific error, and fix.

- [ ] **Step 4: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add demo/src/lib.rs && git commit -m "test(demo): add tuple exercise functions"
```

---

## Task 6: Final verification

**Files:**
- No file changes.

- [ ] **Step 1: Build pipeline produces `.gia`**

Run:
```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: pipeline completes; `target/rust2genshin-demo.gia` exists and is larger than the pre-tuple-support baseline (since the new demo functions add composite nodes).

- [ ] **Step 2: Unit tests pass**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 3: Clippy is clean**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep -E "^warning:" | grep -v "both \`C:\\\\" | grep -v "profiles for the non root" | wc -l
```

Expected: 0 (the 2-3 cargo config/profile environmental warnings are excluded by the grep).

- [ ] **Step 4: `// TODO` markers preserved**

Run:
```bash
cd F:/rust2genshin && grep -rn "// TODO" core/src lib/src demo/src 2>&1 | wc -l
```

Compare the count to the pre-tuple-support state (commit `bf7a82d`). The count should be the same or higher (we should not have removed any TODOs).

- [ ] **Step 5: Report summary**

If all 4 steps pass, the tuple-support sub-project is complete. Five implementation commits are on the branch (Tasks 1-5) plus the doc commits. Note in your final report:

- The new `.gia` file size (it's expected to grow from the baseline due to new composite nodes)
- The list of new struct definitions registered in the bundle (4 — one per unique tuple type used in demos)
- Whether the count of clippy warnings is exactly the same as before tuple work (it should be; we didn't introduce new lints)

If any step fails, report the failure and the relevant commit to investigate.