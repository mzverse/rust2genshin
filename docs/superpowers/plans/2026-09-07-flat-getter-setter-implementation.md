# Flat::getter and Flat::setter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `LocalVar::Flat::getter()` and `LocalVar::Flat::setter()` in `core/src/compile/func.rs` using `NODE_ASSEMBLE_STRUCT` (kernel 300002) and `NODE_SPLIT_STRUCT` (kernel 300003), enabling whole-tuple reads and writes through `let p = t;` and `p = q;` patterns.

**Architecture:** Four sequential tasks. Task 1 implements `Flat::getter` (per the user's request to do whole-tuple get first). Task 2 implements `Flat::setter`. Task 3 updates `compile_operand` to call the new `getter(graph, kind)` signature. Task 4 adds demo functions and verifies the demo pipeline succeeds.

**Tech Stack:** Rust nightly, `rustc_private`, existing `crate::asset::node_graph::arithmetic::{NODE_ASSEMBLE_STRUCT, NODE_SPLIT_STRUCT}`, `crate::asset::value::ValueStruct`, `crate::asset::node_graph::{Connection, Node, ValueIn, Block, NodeGraph}`.

**Project root:** `F:/rust2genshin/`

---

## File Structure

**Modified:**
- `core/src/compile/func.rs` — `LocalVar::Flat::getter`, `LocalVar::Flat::setter`, `compile_operand`.
- `demo/src/lib.rs` — append 4 demo functions.

**Unchanged:**
- `core/src/asset/node_graph/arithmetic.rs` — `NODE_ASSEMBLE_STRUCT` and `NODE_SPLIT_STRUCT` already exist.
- `core/src/asset/value.rs` — `ValueStruct` already encodes `struct_id`.
- `core/proto/asset.proto` — no schema changes.

**No new files.**

---

## Task 1: Implement `LocalVar::Flat::getter` using STRUCT_ASSEMBLY

**Files:**
- Modify: `core/src/compile/func.rs` — `LocalVar::Flat::getter` arm.

- [ ] **Step 1: Find the `Flat` arm of `getter`**

Open `core/src/compile/func.rs`. Find the `impl LocalVar` block. The current `getter()` has:

```rust
pub fn getter(&self) -> Connection {
    match self {
        LocalVar::Basic(x) => Connection(*x, 1),
        LocalVar::Struct { getter, .. } => Connection(*getter, 0),
        LocalVar::Flat(_) => todo!(),
    }
}
```

- [ ] **Step 2: Change the signature to accept `graph` and `kind`**

Change:
```rust
pub fn getter(&self) -> Connection {
```

to:
```rust
pub fn getter(&self, graph: &mut NodeGraph<impl NodeGraphExtra>, kind: AnyValue) -> ValueIn {
```

This signature change ripples to the call site in `compile_operand` (handled in Task 3) and to the recursive call inside `Flat::getter` itself (since it recurses through leaves via `field.getter(...)`).

- [ ] **Step 3: Implement the `Flat` arm**

Replace the `Flat` arm:
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
    use crate::asset::value::{ValueInt, ValueBool};
    use crate::asset::node_graph::arithmetic::NODE_ASSEMBLE_STRUCT;

    let mut node_kind = NODE_ASSEMBLE_STRUCT.clone();
    let field_types: Vec<AnyValue> = match kind.as_ref().downcast_ref::<ValueStruct>() {
        Some(s) => s.fields.clone(),
        None => return ValueIn::value(ValueBool::def()),  // defensive
    };
    let struct_id = match kind.as_ref().downcast_ref::<ValueStruct>() {
        Some(s) => s.struct_id,
        None => 0,
    };
    // Populate input types: struct_id selector (int) + N leaf types.
    let mut input_types: Vec<AnyValue> = vec![ValueInt(0).into()];
    input_types.extend(field_types.iter().cloned());
    node_kind.values_in_types = input_types;
    // Output type is `kind` itself.
    node_kind.values_out_types = vec![kind.clone()];
    let node_ref = graph.insert(node_kind);
    // Wire struct_id selector on input pin 0.
    graph.set_value_in(
        Connection(node_ref, 0),
        ValueIn::value(ValueInt(struct_id as i32).into()),
    );
    // Wire each leaf's getter to the corresponding field input pin (pin i+1).
    for (i, field) in fields.iter().enumerate() {
        let leaf_value = field.getter(graph, ValueBool::def());  // leaf kind is scalar; placeholder
        graph.set_value_in(Connection(node_ref, (i + 1) as i32), leaf_value);
    }
    ValueIn::link(Connection(node_ref, 0).into())
}
```

- [ ] **Step 4: Build (expect errors at call sites)**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build errors. The `compile_operand` call site still uses `getter()` (old signature, no args), and the `LocalVar::Basic` arm still uses `Connection(*x, 1)` but the new return type is `ValueIn`. These will be addressed in Task 3.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): implement Flat::getter using STRUCT_ASSEMBLY"
```

---

## Task 2: Implement `LocalVar::Flat::setter` using STRUCT_SPLIT

**Files:**
- Modify: `core/src/compile/func.rs` — `LocalVar::Flat::setter` arm.

- [ ] **Step 1: Find the `Flat` arm of `setter`**

The current `setter()`:

```rust
pub fn setter(&self, graph: &mut NodeGraph<impl NodeGraphExtra>, kind: AnyValue, value: ValueIn) -> Block {
    match self {
        LocalVar::Basic(x) => {
            let node = graph.insert(node_set_local(kind).into());
            graph.connect_value(Connection(*x, 0), Connection(node, 0));
            graph.set_value_in(Connection(node, 1), value);
            Block::singleton(node, 0)
        }
        LocalVar::Struct { .. } => todo!(),
        LocalVar::Flat(_) => todo!(),
    }
}
```

- [ ] **Step 2: Implement the `Flat` arm**

Replace the `Flat` arm:
```rust
LocalVar::Flat(_) => todo!(),
```

with:
```rust
LocalVar::Flat(fields) => {
    // Insert STRUCT_SPLIT (kernel 300003): takes the struct value and produces
    // per-field outputs. For each leaf, insert a set_local and wire STRUCT_SPLIT's
    // per-field output to the leaf's set_local.
    use crate::asset::node_graph::arithmetic::NODE_SPLIT_STRUCT;
    let field_types: Vec<AnyValue> = match kind.as_ref().downcast_ref::<ValueStruct>() {
        Some(s) => s.fields.clone(),
        None => return Block::nop(graph),  // defensive
    };
    let mut node_kind = NODE_SPLIT_STRUCT.clone();
    node_kind.values_in_types = vec![kind.clone()];
    node_kind.values_out_types = field_types.clone();
    let node_ref = graph.insert(node_kind);
    // Wire the value to the struct input.
    graph.set_value_in(Connection(node_ref, 0), value);
    // For each leaf, insert a set_local and wire STRUCT_SPLIT's output to it.
    let mut block = Block::nop(graph);
    for (i, field) in fields.iter().enumerate() {
        let leaf_kind = field_types[i].clone();
        let block_for_field = field.setter(graph, leaf_kind, ValueIn::link(Connection(node_ref, i).into()));
        block.extend(graph, block_for_field);
    }
    block
}
```

- [ ] **Step 3: Build (expect errors at call sites)**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build errors at the call sites (still using old signatures). Task 3 fixes them.

- [ ] **Step 4: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): implement Flat::setter using STRUCT_SPLIT"
```

---

## Task 3: Update `compile_operand` to use new `getter` signature

**Files:**
- Modify: `core/src/compile/func.rs` — `compile_operand` operand copy/move arm.

- [ ] **Step 1: Find the operand copy/move arm**

The current `compile_operand`:

```rust
fn compile_operand(&mut self, op: &Operand<'tcx>, span: Span) -> Result<ValueIn> {
    Ok(match op {
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
        // ... other arms ...
    })
}
```

- [ ] **Step 2: Update the arm**

Replace:
```rust
ValueIn::link(local.getter().into())
```

with:
```rust
let src_kind = self.compiler.compile_ty(span, p.ty(&self.body.local_decls, self.tcx))?;
local.getter(self.graph, src_kind)
```

This calls the new `getter(graph, kind)` signature. The `src_kind` is the type at the projection depth — scalar for a leaf walk, struct for whole-tuple reads.

- [ ] **Step 3: Build to verify**

Run:
```bash
cd F:/rust2genshin && cargo +nightly build -p rust2genshin
```

Expected: build succeeds. All 4 of the prior error sites are resolved.

- [ ] **Step 4: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add core/src/compile/func.rs && git commit -m "feat(core): update compile_operand to call Flat::getter(graph, kind)"
```

---

## Task 4: Add demo functions and verify end-to-end

**Files:**
- Modify: `demo/src/lib.rs` — append 4 demo functions.

- [ ] **Step 1: Read current end of demo file**

Open `demo/src/lib.rs`. The file should end with the existing `nested_tuple_first` function.

- [ ] **Step 2: Append the 4 demo functions**

Add at the end of `demo/src/lib.rs`:

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

- [ ] **Step 3: Run the demo pipeline**

Run:
```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

Expected: pipeline completes; `target/rust2genshin_demo.gia` exists.

If the pipeline fails, the most likely causes are:
- STRUCT_ASSEMBLY/STRUCT_SPLIT wire format mismatch (selector or input/output types) — investigate by adding `eprintln!` debugging.
- `set_value_in` on input pin 0 of STRUCT_SPLIT not accepting the struct value — may need `set_default` or `connect_value` instead.

- [ ] **Step 4: Run unit tests**

Run:
```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5 pass.

- [ ] **Step 5: Commit**

```bash
cd F:/rust2genshin && git add demo/src/lib.rs && git commit -m "test(demo): add field-write demo functions (swap, copy, update, nested)"
```

- [ ] **Step 6: Run clippy**

Run:
```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep "^warning:" | wc -l
```

Expected: 3-7 (only environmental + possibly the polymorphic-selector warning we discussed in the spec). If many more, investigate.

---

## Final verification

After all 4 tasks complete:

- [ ] **Demo pipeline succeeds**

```bash
cd F:/rust2genshin && cargo +nightly run -p build-demo
```

- [ ] **Tests pass**

```bash
cd F:/rust2genshin && cargo +nightly test -p rust2genshin
```

Expected: 5/5.

- [ ] **No new clippy warnings**

```bash
cd F:/rust2genshin && cargo +nightly clippy --workspace --all-targets 2>&1 | grep "^warning:" | wc -l
```

Expected: similar to pre-task count (3 baseline + new STRUCT_ASSEMBLY selector warning at most).

If any step fails, report the failure and the relevant commit to investigate.