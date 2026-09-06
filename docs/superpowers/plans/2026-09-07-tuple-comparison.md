# Tuple Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable `==` and `!=` on 2-tuples via a `tuple_eq!` macro that expands to field-wise scalar `==` chained with `&&`.

**Architecture:** Four tasks. The original plan's six tasks (backend-side `compare_tuple_values` helper + `BinOp::Eq` branch) were attempted, committed (`2ecb8e7`, `6dd3dd0`), and reverted (`b92df0d`, `1356ab9`) — see "Pivot" below. The shipped approach sidesteps the backend entirely: the macro expands at source level to scalar `==` chains, which MIR lowers to `Rvalue::BinaryOp(Eq, ...)` (not `<T as PartialEq>::eq` trait dispatch), and the backend's existing scalar paths handle every piece.

**Tech Stack:** Rust nightly, `rustc_private`, existing `crate::asset::node_graph::arithmetic::{NODE_AND, node_equal}`, `macro_rules!`, no new backend code.

**Project root:** `F:/rust2genshin/`

**Spec:** `docs/superpowers/specs/2026-09-07-tuple-comparison-design.md` (revised after implementation)

## Pivot from original plan

The original plan's Tasks 3 and 4 (`compare_tuple_values` helper + `BinOp::Eq`/`Ne` branching in `compile_assign_rvalue`) made an architectural assumption that turned out to be wrong: that rustc lowers tuple `p == q` to `Rvalue::BinaryOp(BinOp::Eq, ...)`. It does not — for tuples, rustc emits a trait method call: `_0 = <(A, B) as PartialEq>::eq(move _3, move _4)`. The intermediate locals `_3: &(A, B)` and `_4: &(A, B)` panic in `solve_local` before the `BinOp::Eq` branch ever fires. Those tasks landed and were reverted.

The shipped approach: a `tuple_eq!` macro in `rust2genshin-lib` (commit `813640a`) that expands to field-wise scalar `==` chained with `&&`. Every comparison is between scalar fields, which MIR DOES lower to `Rvalue::BinaryOp(Eq, ...)` (the same path that works for `i32 == i32`). The `&&` lowers to `BinOp::BitAnd(bool)` → `NODE_AND`. No backend changes needed for tuple comparison.

Tasks 1 and 2 from the original plan (`insert_struct_split` extraction + `Flat::setter` migration, commits `a743400` and `b297412`) shipped as planned and remain in place — they're not on the critical path for tuple comparison but improve `Flat::setter`'s maintainability.

---

## File Structure

**Modified:**
- `core/src/compile/func.rs` — add `insert_struct_split` and `compare_tuple_values` helpers; refactor `LocalVar::Flat::setter`; branch `BinOp::Eq`/`Ne` arm in `compile_assign_rvalue`.
- `demo/src/lib.rs` — append 2 demo functions.

**Unchanged:**
- `core/src/asset/node_graph/arithmetic.rs` — `NODE_SPLIT_STRUCT`, `NODE_AND`, `node_equal` already exist.
- `core/src/asset/value.rs` — `ValueStruct` already encodes `struct_id` and `fields`.
- `core/proto/asset.proto` — no schema changes.

**No new files.**

---

## Task 1: Extract `insert_struct_split` helper

**Files:**
- Modify: `core/src/compile/func.rs` — add `insert_struct_split` helper above `impl LocalVar`.

- [ ] **Step 1: Locate the existing `flat_child_kinds` helper**

Open `core/src/compile/func.rs`. The helper sits at lines 84–97 (right above `impl LocalVar`). The new helper goes immediately after it.

- [ ] **Step 2: Add the `insert_struct_split` function**

Insert immediately after `flat_child_kinds` (before `impl LocalVar` at line 99):

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

- [ ] **Step 3: Build the backend to verify the helper compiles**

Run:
```bash
cargo +nightly build -p rust2genshin
```
Expected: build succeeds. The new helper is unused for now (warning may appear — that's fine, refactored in Task 2).

- [ ] **Step 4: Commit**

```bash
git add core/src/compile/func.rs
git commit -m "$(cat <<'EOF'
refactor(core): extract insert_struct_split helper

Pulls the STRUCT_SPLIT (300003) construction out of Flat::setter
into a reusable helper. Same pin-layout pattern (polymorphic
struct in at pin 0, per-field outputs at pins 0..N-1, selectors
resized in lock-step with the values vectors). The setter will
be migrated to this helper in the next task; future callers (e.g.
tuple comparison) share the same shape.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Refactor `Flat::setter` to use `insert_struct_split`

**Files:**
- Modify: `core/src/compile/func.rs` — `LocalVar::Flat::setter` arm.

- [ ] **Step 1: Verify pre-refactor pipeline works**

Run the demo pipeline to establish a baseline:
```bash
cargo +nightly run -p build-demo 2>&1 | tail -5
```
Expected: pipeline completes successfully, `target/rust2genshin_demo.gia` exists.

- [ ] **Step 2: Locate `LocalVar::Flat::setter`**

In `core/src/compile/func.rs`, find the `LocalVar::Flat(fields)` arm of `setter()` (around line 167 after Task 1's insertion). It currently looks like:

```rust
LocalVar::Flat(fields) => {
    // STRUCT_SPLIT (kernel 300003). Pin layout:
    //   - input pin 0 = the struct value (polymorphic ValueStruct)
    //   - output pins 0..N-1 = per-field values (one per tuple element)
    // Pin 0 is dual-purpose: in is the struct, out is the first field.
    // Nested Flat children recurse via their own setter(), so each
    // level only ever sees its own arity.
    use crate::asset::node_graph::arithmetic::NODE_SPLIT_STRUCT;

    let mut node_kind = NODE_SPLIT_STRUCT.clone();
    let field_types = flat_child_kinds(&*kind, fields.len());
    node_kind.values_in_types = vec![kind.clone()];
    node_kind.values_out_types = field_types.clone();
    // `NodeKind::new` sized selectors_in/selectors_out from the prototype's
    // (empty) values_*, so resize both in lock-step (see getter).
    node_kind.selectors_in = vec![None; node_kind.values_in_types.len()];
    node_kind.selectors_in[0] = Some(0);
    node_kind.selectors_out = vec![None; node_kind.values_out_types.len()];
    let node_ref = graph.insert(node_kind.into());
    graph.set_value_in(Connection(node_ref, 0), value);
    let mut block = Block::nop(graph);
    for (i, field) in fields.iter().enumerate() {
        let block_for_field = field.setter(graph, field_types[i].clone(), ValueIn::link(Connection(node_ref, i).into()));
        block.extend(graph, block_for_field);
    }
    block
},
```

- [ ] **Step 3: Replace the inline construction with a call to `insert_struct_split`**

Replace the `LocalVar::Flat(fields)` arm of `setter()` with:

```rust
LocalVar::Flat(fields) => {
    // STRUCT_SPLIT (kernel 300003). Pin layout:
    //   - input pin 0 = the struct value (polymorphic ValueStruct)
    //   - output pins 0..N-1 = per-field values (one per tuple element)
    // Pin 0 is dual-purpose: in is the struct, out is the first field.
    // Nested Flat children recurse via their own setter(), so each
    // level only ever sees its own arity.
    let struct_kind = match kind.downcast_ref::<ValueStruct>() {
        Ok(vs) => vs.clone(),
        Err(_) => return Block::nop(graph),
    };
    let field_types = struct_kind.fields.clone();
    let node_ref = insert_struct_split(graph, &struct_kind, value);
    let mut block = Block::nop(graph);
    for (i, field) in fields.iter().enumerate() {
        let block_for_field = field.setter(graph, field_types[i].clone(), ValueIn::link(Connection(node_ref, i).into()));
        block.extend(graph, block_for_field);
    }
    block
},
```

- [ ] **Step 4: Build and run the demo pipeline to verify no regressions**

Run:
```bash
cargo +nightly build -p rust2genshin && cargo +nightly run -p build-demo 2>&1 | tail -10
```
Expected: backend compiles (no warnings about unused imports — `NODE_SPLIT_STRUCT` and `flat_child_kinds` should now be unused inside the setter arm; if the compiler warns about either, remove the `use` and any remaining unused references). Pipeline produces `target/rust2genshin_demo.gia` successfully.

- [ ] **Step 5: Commit**

```bash
git add core/src/compile/func.rs
git commit -m "$(cat <<'EOF'
refactor(core): migrate Flat::setter to insert_struct_split helper

Behavior is preserved in the happy path (where `kind` is the
ValueStruct returned by compile_ty for tuple types — the only
reachable case from compile_assign). In the unreachable defensive
path where `kind` is not a ValueStruct, behavior changes from
"build a broken STRUCT_SPLIT with ValueBool placeholders" to
"return nop" — strictly an improvement.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Add `compare_tuple_values` helper

**Files:**
- Modify: `core/src/compile/func.rs` — add `compare_tuple_values` helper.

- [ ] **Step 1: Add the helper function**

Insert immediately after `insert_struct_split` (the function added in Task 1):

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

- [ ] **Step 2: Build the backend**

Run:
```bash
cargo +nightly build -p rust2genshin
```
Expected: builds. The helper is unused — a dead-code warning may appear (this is fine, wired in Task 4).

- [ ] **Step 3: Commit**

```bash
git add core/src/compile/func.rs
git commit -m "$(cat <<'EOF'
feat(core): add compare_tuple_values helper

Recursively decomposes two struct-shaped values via STRUCT_SPLIT
(300003), compares each field with the existing node_equal, and
folds results with NODE_AND. Handles nested tuples by recursing
when a field is itself a ValueStruct. Helper is unused until
the BinOp::Eq/Ne branch is added.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Branch `BinOp::Eq`/`Ne` to use `compare_tuple_values`

**Files:**
- Modify: `core/src/compile/func.rs` — `BinOp::Eq | BinOp::Ne` arm at line 288.

- [ ] **Step 1: Locate the Eq/Ne arm**

In `core/src/compile/func.rs`, find the line:
```rust
BinOp::Eq | BinOp::Ne => node_equal(kind0),
```
at line 288.

- [ ] **Step 2: Replace with branching logic**

Replace the single line with:

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

**Important:** the existing code immediately after this arm (lines 298–304) calls `self.compile_operand(&v.0, span)?` and `self.compile_operand(&v.1, span)?` to populate the comparison node's inputs. After the change, the tuple path compiles the operands inside the branch (since the helper consumes `ValueIn`s, not `Operand`s); the scalar path still relies on the outer `compile_operand` calls. The outer `let a = ...` and `let b = ...` lines (lines 298–299) now need to be guarded or restructured.

Read lines 296–311 of the current file. The structure is:

```rust
let mut node = self.graph.insert(Node::new(match op {
    // ... arms above produce a NodeKind ...
    BinOp::Eq | BinOp::Ne => node_equal(kind0),
    // ... arms below ...
}));
let a = self.compile_operand(&v.0, span)?;
let b = self.compile_operand(&v.1, span)?;
self.graph.set_value_in(Connection(node, 0), a);
self.graph.set_value_in(Connection(node, 1), b);
if matches!(op, BinOp::Ne) {
    let not_node = self.graph.insert(Node::new(NODE_NOT.clone()));
    self.graph.connect_value(Connection(node, 0), Connection(not_node, 0));
    node = not_node;
}
ValueIn::link(Connection(node, 0).into())
```

The tuple path needs to return early BEFORE the `let mut node = ...` line, since there's no single comparison `node` to populate. Restructure the match arm so the tuple path returns directly:

Replace lines 273–311 (the entire `Rvalue::BinaryOp(op, v) => { ... }` block) with:

```rust
Rvalue::BinaryOp(op, v) => {
    let ty0 = v.0.ty(&self.body.local_decls, self.tcx);
    let kind0 = self.compiler.compile_ty(v.0.span(&self.body.local_decls), ty0)?;
    // Tuple equality short-circuits the normal node-construction flow because
    // there's no single comparison node — we emit a STRUCT_SPLIT-based
    // decomposition that returns a bool ValueIn directly.
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        if let Ok(vs) = kind0.downcast_ref::<ValueStruct>() {
            let lhs_v = self.compile_operand(&v.0, span)?;
            let rhs_v = self.compile_operand(&v.1, span)?;
            let mut eq_value = compare_tuple_values(self.graph, lhs_v, rhs_v, vs);
            if matches!(op, BinOp::Ne) {
                let not_node = self.graph.insert(Node::new(NODE_NOT.clone()));
                let eq_conn = eq_value.link.unwrap().connection().unwrap();
                self.graph.connect_value(eq_conn, Connection(not_node, 0));
                eq_value = ValueIn::link(Connection(not_node, 0).into());
            }
            return self.compile_assign(place, eq_value);
        }
    }
    let mut node = self.graph.insert(Node::new(match op {
        BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow =>
            node_add(kind0),
        BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow =>
            node_subtract(kind0),
        BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow =>
            node_multiply(kind0),
        BinOp::Div => node_divide(kind0),
        BinOp::Rem => NODE_MODULO.clone(),
        BinOp::BitXor => if ty.is_bool() { NODE_XOR.clone() } else { NODE_BITWISE_XOR.clone() },
        BinOp::BitAnd => if ty.is_bool() { NODE_AND.clone() } else { NODE_BITWISE_AND.clone() },
        BinOp::BitOr => if ty.is_bool() { NODE_OR.clone() } else { NODE_BITWISE_OR.clone() },
        BinOp::Eq | BinOp::Ne => node_equal(kind0),
        BinOp::Shl | BinOp::ShlUnchecked => NODE_LEFT_SHIFT.clone(),
        BinOp::Shr | BinOp::ShrUnchecked => return self.compile_call(span, self.compiler.find_lib_fn("<i32 as rust2genshin_lib::math::I32>::shr")?, &[dummy_spanned(v.0.clone()), dummy_spanned(v.1.clone())], place),
        BinOp::Lt => node_less_than(kind0),
        BinOp::Le => node_less_equal(kind0),
        BinOp::Ge => node_greater_equal(kind0),
        BinOp::Gt => node_greater_than(kind0),
        | BinOp::Cmp
        | BinOp::Offset => todo!("{:?}", op),
    }));
    let a = self.compile_operand(&v.0, span)?;
    let b = self.compile_operand(&v.1, span)?;
    self.graph.set_value_in(Connection(node, 0), a);
    self.graph.set_value_in(Connection(node, 1), b);
    if matches!(op, BinOp::Ne) {
        // ! (a == b) — invert the equal node's bool output
        let not_node = self.graph.insert(Node::new(NODE_NOT.clone()));
        self.graph.connect_value(Connection(node, 0), Connection(not_node, 0));
        node = not_node;
    }
    ValueIn::link(Connection(node, 0).into())
}
```

Notes on the restructure:
- The tuple path calls `return self.compile_assign(place, eq_value)` early — it doesn't fall through to the standard `node` construction. This avoids creating an unused scalar equal node.
- For `Ne`, the helper's bool output is inverted via `NODE_NOT`. `eq_value.link.unwrap().connection().unwrap()` extracts the `Connection` from the helper's `ValueIn::link(...)` return value, which is safe because `compare_tuple_values` always returns `ValueIn::link(...)` (never a plain default value).
- The scalar path is unchanged from the original — `BinOp::Eq | BinOp::Ne => node_equal(kind0)` is still in the inner match.

- [ ] **Step 3: Build the backend**

Run:
```bash
cargo +nightly build -p rust2genshin
```
Expected: compiles. The warning about `compare_tuple_values` being unused should be gone (it's now called from the BinOp arm).

- [ ] **Step 4: Commit**

```bash
git add core/src/compile/func.rs
git commit -m "$(cat <<'EOF'
feat(core): branch BinOp::Eq/Ne to decompose tuple comparison

When kind0 is a ValueStruct, route through compare_tuple_values
instead of node_equal (which panics on struct types). The scalar
path is unchanged. Tuple Ne wraps the bool result in NODE_NOT to
match the existing scalar Ne handling. Returns early via
compile_assign so the standard node-construction path doesn't
fire for tuples.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Add demo functions

**Files:**
- Modify: `demo/src/lib.rs` — append 2 demo functions at the end.

- [ ] **Step 1: Locate the end of the demo file**

Open `demo/src/lib.rs`. The file ends with `nested_update` (around line 120). Append after it.

- [ ] **Step 2: Add `tuple_eq` and `nested_tuple_eq`**

Append the following to the end of `demo/src/lib.rs`:

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

- [ ] **Step 3: Build the backend**

Run:
```bash
cargo +nightly build -p rust2genshin
```
Expected: builds. (Adding demo functions doesn't affect backend compilation — they're part of the leaf crate.)

- [ ] **Step 4: Commit**

```bash
git add demo/src/lib.rs
git commit -m "$(cat <<'EOF'
test(demo): add tuple_eq and nested_tuple_eq demo functions

Exercises flat and nested tuple equality end-to-end. Both are
#[unsafe(no_mangle)] so they're registered as composite nodes
in the produced .gia. tuple_eq covers the 2-tuple decomposition
path; nested_tuple_eq covers recursion (outer split's index-0
field is itself a struct, requiring a recursive compare).

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Verify the full pipeline

**Files:**
- None — verification only.

- [ ] **Step 1: Run the full build pipeline**

Run:
```bash
cargo +nightly run -p build-demo 2>&1 | tail -20
```
Expected: pipeline completes successfully. No panics, no `Type error`, no `index out of bounds` errors. The output should mention "Finished" and exit cleanly.

- [ ] **Step 2: Verify the .gia artifact exists**

Run:
```bash
ls -la target/rust2genshin_demo.gia
```
Expected: file exists, size is non-trivial (modest growth vs pre-spec, expected to be in the low tens of KB based on prior demo functions).

- [ ] **Step 3: Spot-check the artifact for the new kernel IDs**

Run:
```bash
strings target/rust2genshin_demo.gia 2>/dev/null | grep -E "300003|NODE_AND|node_equal" | head -5
```
Expected: hits on STRUCT_SPLIT (300003), NODE_AND, and node_equal kernel patterns. This confirms the decomposition actually fired for the new demo functions.

If any of these checks fail, the implementation has a bug — return to the task that introduced the regression (likely Task 4 if the panic is in compile_assign_rvalue, or Task 2 if it's in Flat::setter).

- [ ] **Step 4: Final commit (no code changes)**

If the previous commits are sufficient, no commit is needed. If you found and fixed any verification-time issues, commit them now:
```bash
git status
git add <any changed files>
git commit -m "fix: <describe any verification-time fix>"
```

(If nothing was changed, skip this step.)

- [ ] **Step 5: Push to remote**

```bash
git push origin main
```
Expected: push succeeds; branch is up to date with origin/main.

---

## Self-Review Notes (planning-time)

- **Spec coverage:**
  - Spec Change 1 (`insert_struct_split` helper) → Task 1 ✓
  - Spec Change 2 (`compare_tuple_values` helper) → Task 3 ✓
  - Spec Change 3 (Branch in `BinOp::Eq`/`Ne`) → Task 4 ✓
  - Spec Change 4 (`Flat::setter` refactor) → Task 2 ✓
  - Spec Change 5 (Demo functions) → Task 5 ✓
  - Spec Testing section → Task 6 ✓
- **No placeholders:** every code block is complete; no "TODO" or "TBD" markers.
- **Type consistency:** `insert_struct_split(graph, &ValueStruct, ValueIn) -> NodeRef` matches in Tasks 1, 2, 3. `compare_tuple_values(graph, ValueIn, ValueIn, &ValueStruct) -> ValueIn` matches in Tasks 3, 4. `BinOp::Eq | BinOp::Ne` pattern matches the spec.
- **Scope:** Single feature (tuple comparison). Ordering and struct comparison remain out of scope per the spec.
