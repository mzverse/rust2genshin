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

改接时目标引脚要的是 `PartialMut`，而写入值是普通值 —— 中间补一层 `ToPartialMut`（`T` 进，`PartialMut<T>` 出，无控制流）。读的那一侧原本就带着 `Copy`，两层互逆，`eliminate_copy` 把 `Copy(ToPartialMut(x))` 消成直连。

= `&mut` 结构体字段

`s.field` 的 `&mut` 编译成 `FieldMut`：输出 `Mut<字段>`，setter 那头在节点图里没有对应物。优化期把直接经它的写入换成 `Modify`、把直接读换成解构 owner 的 `PartialMut`；中转留给以后的内联优化。没消掉的 `FieldMut` 降级时 panic。
