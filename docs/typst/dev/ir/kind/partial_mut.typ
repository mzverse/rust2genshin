#import "/lib/lib.typ": *;
#show: template;

#let title = [PartialMut];
#let tag = <tag-partial-mut>

#set raw(lang: "rust");

可变视图：值就是被视图对象的值，但标记着「这是通过引用拿到的」

局部变量交出来的永远是视图，读值要过`Copy`节点

lower后等同于被视图的类型

= 借用

- `&T`：`T` 实现 `Freeze` 时就是 `T`，否则是 `PartialMut<T>`
- 已经是 `PartialMut` 的不会再包一层（`&PartialMut<T>` 仍是视图）

`Freeze` 时的 `&T` *不扁平化*：`&(i32, i32)` 是一个 ADT 局部变量，不是两个槽。对这种 `T` 的局部变量取不可变借用等同于 Move / Copy —— 读到的是值，走 `Copy`。

`&mut T` 见#link(<tag-mut>)[Mut]。

= lib 里的 PartialMut

`PartialMut<T>` 也可以直接写进 lib，让「通过引用写」在类型上说得清。三个内建不走 `compile_native_call`，在 `compile_call` 特判：

- `partial_mut_from` —— 拆 `&mut T` 取读槽
- `partial_mut_deref` —— 值直通
- `partial_mut_deref_mut` —— 视图里没有 setter 槽，拼不回来；输出元组的那一格给 `Unsupported`，降级时报出来
