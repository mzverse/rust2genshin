#import "/lib/lib.typ": *;
#show: template;

#let title = [类型];

#set raw(lang: "rust");

类型的中间表示。`IrKind::Native` 是能直接落成节点图引脚的原生类型，其余变体都是
*编译期才会存在*的形态 —— 降级时遇到就 panic。

= 原生类型

带 `#[native("名字")]` 的类型由 `lib/` 声明，编译器按名字对上（`Guid` / `Entity` /
`Faction` / `Config` / `Prefab` / `Vec3` / `PartialMut` / `List` / `Dict`）。

泛型的（`List<T>`、`PartialMut<'a, T>`）从*调用点*的泛型实参取元素类型。注意
`PartialMut<'a, T>` 的第一个实参是生命周期，要按顺序挑出类型参数。

用名字而不是 def-id 来对齐，是为了让 `lib/` 能在不改 core 的情况下新增原生类型 ——
代价是名字写错只能在编译时靠 `unknown native type` 报出来，没有类型检查兜着。

= 列表

`[T; N]` 编译成 `List(T)`，长度不进类型 —— 所以 `[i32; 3]` 与 `List<i32>` 同型，
两者之间的 `transmute` 是空转换。

长度为什么不进类型：节点图的列表节点是泛型变体（shell 固定、kernel 随元素类型变），
没有「定长数组」这一档。要保住定长语义就得在图上多存一个长度约束节点，而没有任何
节点会读它。

= 元素不是原生类型时

节点的列表元素只认原生类型和结构体。元素本身是列表 / 字典时，降级时包一层单字段
结构体再放进去（`IrKind::lower_list_elem`，`core/src/compile/ir.rs:235`）。

包一层而不是直接拒绝，是因为「列表的列表」在 Rust 里完全合法，直接报错会让用户
无从下手。代价是每次读写都要多一个 assemble / destructure —— 这正是
`nested_list_demo` 在产物里比 `List<i32>` 多出节点的原因。

`LocalRef` 对没有 local 变体的类型做的是同一件事。

= 各变体

#list(
    link("/arch/ir/kind/mut.html", [Mut]) + [ —— `&mut T` 为什么是两个槽],
    link("/arch/ir/kind/partial_mut.html", [PartialMut]) + [ —— 可变视图，以及它为什么不能存],
    link("/arch/ir/kind/never.html", [Never]) + [ —— `!`，只在类型编译阶段存在],
)
