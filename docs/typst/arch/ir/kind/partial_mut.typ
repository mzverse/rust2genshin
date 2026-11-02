#import "/lib/lib.typ": *;
#show: template;

#let title = [PartialMut];
#let tag = <tag-partial-mut>

#set raw(lang: "rust");

可变视图：值就是被视图对象的值，但标记着「这是通过引用拿到的」

局部变量交出来的永远是视图，读值要过 `Copy` 节点

lower 后等同于被视图的类型

= 为什么需要它

节点图里的「写」不是通过指针完成的 —— 写元素走的是 `list_set` / `Modify` 这类
*执行节点*，它们直接改原处，不需要句柄。所以 `&mut T` 在图上并没有一个「指向
T 的写句柄」可以传。

`PartialMut<T>` 就是这件事的诚实表达：值一样（lower 后等同于 `T`），但类型上
标着「这是通过引用拿到的、要写就得走执行节点」。它不承诺能整体赋值 —— 那需要
真句柄。

这也是它*不可储存*的来源：一块可写视图有意义的前提是原处在附近，存进变量或者
跨控制流之后，原处还在不在就说不清了。

= 借用

- `&T`：`T` 实现 `Freeze` 时就是 `T`，否则是 `PartialMut<T>`
- 已经是 `PartialMut` 的不会再包一层（`&PartialMut<T>` 仍是视图）

`Freeze` 时的 `&T` *不扁平化*：`&(i32, i32)` 是一个 ADT 局部变量，不是两个槽。
对这种 `T` 的局部变量取不可变借用等同于 Move / Copy —— 读到的是值，走 `Copy`。

用 `Freeze` 而不是「有没有 `&mut`」来分类，是因为它在 rustc 里本来就叫这个名字：
不可变借用能不能当值用，取决于被借的类型里有没有 `UnsafeCell`。

`&mut T` 见#link(<tag-mut>)[Mut]。

= lib 里的 PartialMut

`PartialMut<T>` 也可以直接写进 lib，让「通过引用写」在类型上说得清。三个内建不走
`compile_native_call`，在 `compile_call` 特判：

- `partial_mut_from` —— 拆 `&mut T` 取读槽
- `partial_mut_deref` —— 值直通
- `partial_mut_deref_mut` —— 视图里没有 setter 槽，拼不回来；输出元组的那一格给
  `Unsupported`，降级时报出来

特判而不是走普通原生调用，是因为 `partial_mut_from` 要*拆开*一个元组取其中的读槽，
不是把入参原样变成节点的入引脚。`partial_mut_deref_mut` 拼不回来则是结构性的：
它要交出一个 `&mut T`，而 `&mut T` 在 IR 里是 `(setter, 视图)` 两格。
