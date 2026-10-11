#import "/lib/lib.typ": *;
#let title = [局部变量];
#show: template;

#set raw(lang: "rust");

MIR 的 `Local` 和节点图的局部变量对不上：节点图的 local 节点（`core/src/node/query.rs:278`）只支持一小组标量类型，`Struct` 不在其中。所以*复合类型的局部变量必须被打散成若干标量槽*。

这就是 `core/src/compile/place.rs` 的全部内容。

= CompiledLocal 是一棵树

```rust
pub enum CompiledLocal<T> {
    Singleton(T),                    // 一个标量槽
    Flat(Vec<CompiledLocal<T>>),     // 展平成若干子槽
}
```

`solve_local`（`place.rs:64`）按类型决定展不展：

- `Never` → 空 `Flat`
- `Tuple` → 逐元素展，名字带下标（`x.0`、`x.1`）
- `Closure` → 展成 upvar 元组的形状
- `transparent` 的 `Adt`（非原生类型）→ 展到内层那唯一一个字段
- 带 `#[event]` 标的结构体 → 逐字段展
- 其余 → `Singleton`

*展平是递归的*，所以嵌套元组 `(i32, (f32, bool))` 展成三层树。闭包参数因此变成 `#closure + args` 的双层结构。

= 一个槽 = 一对引脚

`LocalRef { setter, getter }`（`place.rs:31`）—— 对应 local 节点的两个引脚。

`LocalKind` 决定它挂不挂导出：

- `Ret` → 推一个 `export_value_out`
- `Arg` → 推一个 `export_value_in`
- `Other` → 不导出

= 读写靠 assemble / destructure

`CompiledPlace`（`place.rs:45`）描述一个*位置*：

```rust
pub enum CompiledPlace {
    Local(CompiledLocal<LocalRef>),
    Field(Box<CompiledPlace>, AdtKey, usize),   // struct/tuple 的第 usize 个字段
}
```

*读*（`getter`）：`Singleton` 直接取引脚；`Flat` 用 assemble 节点把子槽拼成一个值；`Field` 用 destructure 节点拆出第 N 个字段。

*写*（`setter`）：`Singleton` 走 `node_ir_set_local`；`Flat` 先 destructure 再逐个子槽写；`Field` 走 `node_ir_modify_struct`（只改一个字段，其余原样带过）。

三种结构节点的构造在 `core/src/compile/ir.rs:214-229`（`node_ir_assemble` / `node_ir_destructure` / `node_ir_modify_struct`），底层是 `core/src/structure.rs` 里的 `StructureDefinition`。字段列表由 `struct_fields` 给出，展平树的形状必须和它逐位对应。

= 为什么不把整个元组存成一个局部变量

节点图没有「结构体类型的局部变量」这种节点。`node_local` 的类型表是固定的（`query.rs:280-...`），加不进去。展平是唯一可行的做法 —— 代价是每个复合类型局部变量会变成 N 个子槽，函数引脚数随之翻倍。

元组 `==` 也因此用不了：rustc 把 `p == q` lower 成 `<(A, B) as PartialEq>::eq(...)` 的 trait 调用，而后端不建模 trait dispatch（`compile_call` → `touch_fn` 对 stdlib 函数无效）。见#link("/user/unsupported.html")[不被支持的特性]。
