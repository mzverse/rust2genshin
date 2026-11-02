#import "/lib/lib.typ": *;
#show: template;

#let title = [类型];

#set raw(lang: "rust");

`IrKind` 的变体分两类：能直接落成节点图引脚的*原生类型*（`IrKind::Native`），和只在编译期存在的中间形态。

= 原生类型

带 `#[native("名字")]` 的类型由 `lib/` 声明，编译器按名字对上（`Guid` / `Entity` / `Faction` / `Config` / `Prefab` / `Vec3` / `PartialMut` / `List` / `Dict`）。

泛型的（`List<T>`、`PartialMut<'a, T>`）从*调用点*的泛型实参取元素类型。注意 `PartialMut<'a, T>` 的第一个实参是生命周期，要按顺序挑出类型参数。

用名字而不是 def-id 来对齐，是为了让 `lib/` 能在不改 core 的情况下新增原生类型 —— 代价是名字写错只能在编译时靠 `unknown native type` 报出来，没有类型检查兜着。

= 列表

`[T; N]` 编译成 `List(T)`，长度不进类型 —— 所以 `[i32; 3]` 与 `List<i32>` 同型，两者之间的 `transmute` 是空转换。

长度为什么不进类型：节点图的列表节点是泛型变体（shell 固定、kernel 随元素类型变），没有「定长数组」这一档。要保住定长语义就得在图上多存一个长度约束节点，而没有任何节点会读它。

元素本身是列表 / 字典时，降级会包一层单字段结构体再放进去 —— 能写，但每次读写多一个 assemble / destructure。

= 其余变体

中间形态里只有 `PartialMut` 和 `Mut` 值得单独讲，它们在#link("/arch/ir/index.html")[IR]页的「读值」「`&mut` 结构体字段」两节里说清了；`Never` 只是 `!` 的占位。
