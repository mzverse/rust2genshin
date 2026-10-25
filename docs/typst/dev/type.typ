#import "/lib/lib.typ": *;
#show: template;

#let title = [类型];

#set raw(lang: "rust");

Rust 的类型先编译成 `IrKind`（IR 层），`lower` 之后才成为节点图里的 `NativeKind`。

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

= 列表

`[T; N]` 编译成 `List(T)`，长度不进类型 —— 所以 `[i32; 3]` 与 `List<i32>` 同型，两者之间的 `transmute` 是空转换。

元素只能是节点图里有列表变体的原生类型；结构体、枚举、嵌套列表在编译期报错。

= 原生类型

带 `#[native("名字")]` 的类型由 lib 声明，编译器按名字对上（`Guid` / `Entity` / `Faction` / `Config` / `Prefab` / `Vec3` / `PartialMut` / `List` / `Dict`）。

泛型的（`List<T>`、`PartialMut<'a, T>`）从*调用点*的泛型实参取元素类型。注意 `PartialMut<'a, T>` 的第一个实参是生命周期，要按顺序挑出类型参数。

= 借用

- `&T`：`T` 实现 `Freeze` 时就是 `T`，否则是 `PartialMut<T>`
- `&mut T`：扁平化成 `(LocalSetter, PartialMut<T>)`，`LocalSetter` 只能设置整个变量

`Freeze` 时的 `&T` *不扁平化*：`&(i32, i32)` 是一个 ADT 局部变量，不是两个槽。

对这种 `T` 的局部变量取不可变借用等同于 Move / Copy —— 读到的是值，走 `Copy`。

已经是视图的不会再包一层（`&PartialMut<T>` 仍是视图）。

= 读值：Copy

#link(<tag-local>)[Local] 的两个出口是 `LocalRef(T)`（setter）和 `PartialMut<T>`（值）—— 局部变量交出来的*永远是视图*。

读值要显式经过 `Copy`（`PartialMut<T>` 进，`T` 出，无控制流），降级时等价于值直通。插入点在*读操作数*那一步：

- `Operand::Move` / `Operand::Copy` —— 读值，过 `Copy`（解引用取值也是）
- `Rvalue::Ref`（共享借用）—— 要的是视图，不过 `Copy`

同一个 place，读值和借用给的形态不同，这就是 `Copy` 存在的意义。

= 优化期的读写区分

`eliminate_unnecessary_local_setter` 要判断一个执行节点是「读」还是「通过引用写」。判据就是路径上有没有经过 `Copy`：

- 经过 `Copy` 到达执行节点 —— 读值，这处 usage 可以改接到写入值上；
- 不经 `Copy` 直接到达 —— 拿到的是视图，通过引用写，不算 usage。

自定义复合节点（`Fn`）对值的处理说不清，值流上遇到就放弃整次优化。

改接时目标引脚要的是视图，而写入值是普通值 —— 中间补一层 `ToPartialMut`（`T` 进，`PartialMut<T>` 出，无控制流）。读的那一侧原本就带着 `Copy`，两层互逆，`eliminate_copy` 把 `Copy(ToPartialMut(x))` 消成直连。

= `&mut` 结构体字段

`s.field` 的 `&mut` 编译成 `FieldMut`：输出 `Mut<字段>`，其中 setter 那头在节点图里没有对应物。优化期沿句柄流找落点 —— 句柄可以解构、存进局部变量再取出来 —— 每一处通过它写的 `SetLocal` 换成 `Modify`，读字段的一侧改成解构 owner 的视图。没消掉的 `FieldMut` 降级时 panic。

= PartialMut 作为 lib 的类型

`PartialMut<T>` 也可以直接写进 lib，让「通过引用写」在类型上说得清。三个内建不走 `compile_native_call`，在 `compile_call` 特判：

- `partial_mut_from` —— 拆 `&mut T` 取读槽
- `partial_mut_deref` —— 值直通
- `partial_mut_deref_mut` —— 视图里没有 setter 槽，拼不回来；输出元组的那一格给 `Unsupported`，降级时报出来
