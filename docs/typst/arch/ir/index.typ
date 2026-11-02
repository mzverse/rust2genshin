#import "/lib/lib.typ": *;
#show: template;

#let title = [IR];

#set raw(lang: "rust");

Rust 的类型先编译成 `IrKind`，`lower` 之后才成为节点图里的 `NativeKind`。

```rust
pub enum IrKind {
    Native(NativeKind),       // 原生类型
    List(Box<IrKind>),        // 列表；元素也是 IR 类型
    Never,
    LocalRef(Box<IrKind>),    // 局部变量的 setter 槽
    Mut(Box<IrKind>),         // &mut T：(LocalSetter, PartialMut<T>)
    Adt(AdtKey),
    PartialMut(Box<IrKind>),  // 可变视图
    Unsupported(String),
}
```

除 `Native` 外都是编译期的中间形态：*必须在降级前消掉*，降级时遇到会 panic。

这不是洁癖：节点图的局部变量表只认一小组标量类型
（`core/src/value.rs` 的 `NativeKind`），结构体类型压根没有对应节点。
所以「IR 有中间形态」不是可选项 —— 中间形态必须存在，也就必须在下游被消化掉。

`IrKind::is_storable`（`core/src/compile/ir.rs:89`）回答的是另一个问题：
这个类型能不能当局部变量的存储类型。`PartialMut<T>` 只有在 `T` 本身不可变
（`!is_adt`）时才行 —— 视图持有的是读取路径，不是数据本身。

= 读值：Copy

#link(<tag-local>)[Local] 的两个出口是 `LocalRef(T)`（setter）和 `PartialMut<T>`（值）—— 局部变量交出来的永远是#link(<tag-partial-mut>)[`PartialMut`]。

读值要显式经过 `Copy`（`PartialMut<T>` 进，`T` 出，无控制流），降级时等价于值直通。插入点在*读操作数*那一步：

- `Operand::Move` / `Operand::Copy` —— 读值，过 `Copy`（解引用取值也是）
- `Rvalue::Ref`（共享借用）—— 要的是 `PartialMut`，不过 `Copy`

同一个 place，读值和借用给的形态不同，这就是 `Copy` 存在的意义。

= 优化期怎么区分读写

`eliminate_unnecessary_local_setter` 判断一个执行节点是「读」还是「通过引用写」，判据是路径上有没有经过 `Copy`：

- 经过 `Copy` 到达 —— 读值，这处 usage 可以改接到写入值上；
- 不经 `Copy` 到达 —— 拿到的是 `PartialMut`，通过引用写，不算 usage。

自定义复合节点（`Fn`）对值的处理说不清，值流上遇到就放弃整次优化。

之所以要靠一层节点来区分，是因为「有没有经过 `Copy`」这句话在图上没有别的
载体 —— 引脚类型到处都是 `PartialMut<T>`，路径拓扑本身分不出读和写。

改接时目标引脚要的是 `PartialMut`，而写入值是普通值 —— 中间补一层 `ToPartialMut`（`T` 进，`PartialMut<T>` 出，无控制流）。读的那一侧原本就带着 `Copy`，两层互逆，`eliminate_copy` 把 `Copy(ToPartialMut(x))` 消成直连。

= `&mut` 结构体字段

`s.field` 的 `&mut` 由两半拼成：`FieldSetter<idx>` 吃 `PartialMut<Owner>`、交出 `LocalRef<字段>`（setter 那头在节点图里没有对应物），读的那半是 owner 的普通解构，取第 `idx` 个出口得到 `PartialMut<字段>`。优化期把直接经它的写入换成 `Modify`、把直接读换成同一个解构；中转留给以后的内联优化。没消掉的 `FieldSetter` 降级时 panic。

拼成两半正是因为它真的有两半：给字段赋值和读字段在节点图里走的是两条
完全不同的路（`Modify` 是执行节点，`Destructure` 是纯值节点），没有一个
能同时承担两者。

= 节点

IR 层的节点 id 是 `IrNodeId`（`core/src/compile/ir.rs:252`），它比
`NativeNodeId` 多了几个只在优化期存在的名字：`Local` / `SetLocal` /
`Assemble` / `Destructure` / `Modify` / `FieldSetter` / `Copy` /
`ToPartialMut` / `ListAssemble`。

#list(
    link("/arch/ir/node/local.html", [Local]) + [ —— 局部变量的两个出口],
    link("/arch/ir/node/set_local.html", [SetLocal]) + [ —— 写局部变量，以及它的消除判据],
    link("/arch/ir/node/switch.html", [switch]) + [ —— `if` / `match` 的分支源],
    link("/arch/ir/node/nop.html", [nop]) + [ —— 只为接起控制流而存在的空节点],
    link("/arch/ir/node/unreachable.html", [unreachable]) + [ —— 不可达分支],
)

= 类型

#list(
    link("/arch/ir/kind/index.html", [kind]) + [ —— 原生类型与列表],
    link("/arch/ir/kind/mut.html", [Mut]) + [ —— `&mut T` 的两个槽],
    link("/arch/ir/kind/partial_mut.html", [PartialMut]) + [ —— 可变视图],
    link("/arch/ir/kind/never.html", [Never]) + [ —— `!`],
)
