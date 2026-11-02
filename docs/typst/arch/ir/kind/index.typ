#import "/lib/lib.typ": *;
#show: template;

#let title = [kind];

#set raw(lang: "rust");

类型的中间表示，`IrKind::Native`表示原生类型

包含一些无法lower的类型，相关节点必须在优化时消除掉

= 原生类型

带 `#[native("名字")]` 的类型由 lib 声明，编译器按名字对上（`Guid` / `Entity` / `Faction` / `Config` / `Prefab` / `Vec3` / `PartialMut` / `List` / `Dict`）。

泛型的（`List<T>`、`PartialMut<'a, T>`）从*调用点*的泛型实参取元素类型。注意 `PartialMut<'a, T>` 的第一个实参是生命周期，要按顺序挑出类型参数。

= 列表

`[T; N]` 编译成 `List(T)`，长度不进类型 —— 所以 `[i32; 3]` 与 `List<i32>` 同型，两者之间的 `transmute` 是空转换。

元素只能是节点图里有列表变体的原生类型；结构体、枚举、嵌套列表在编译期报错。
