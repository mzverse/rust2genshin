#import "/lib/lib.typ": *;
#let title = [待办];
#show: template;

#set raw(lang: "rust");

当前还没做、但计划要做的东西。想做贡献的话这里也是一份现成的工作清单。

这份清单是项目自己的路线图，可能滞后于代码 —— 个别条目已经有部分实现（比如 `List` 已经能用，但 `[T; N]` 还没接上）。以实际能否编译为准。

= 语言特性

- `enum`（常量部分）
- `loops`
- `async fn`（协程）
- 客户端节点图

= 类型

- `VarSnapshotRef`
- `List<T>` 和 `[T]`
- `Dict<K, V>`
- `Box<T>`
- 无符号整数
- `i64`

= 原生节点

待完善。

= 原生枚举

暂不支持 `match`。待完善。

= 事件

待完善。

= 其它

`inline`。
