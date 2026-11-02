#import "/lib/lib.typ": *;
#let title = [待办];
#show: template;

#set raw(lang: "rust");

还没做、但计划要做的。这份清单可能滞后于代码 —— 以能不能编译为准。

= 语言特性

- `enum` 的常量部分（`match` 还不支持）
- `Shr`（右移）—— 会 panic，不是静默错误
- `async fn` 的完整调度
- 客户端节点图

= 类型

- `VarSnapshotRef` —— 变量快照，用于取事件发生时的值
- `Dict<K, V>` —— 类型和原生节点都在，缺读写 API
- `Box<T>` 的写（`*b = v`、`m.x = v`）
- `i8` / `i16` / `i64` 等其它整数宽度
- `u64` / `u128`
- `f64` 等其它浮点宽度
- `char`

= 原生节点

节点构造大多是齐的，缺的是把它们接进 `lib/`（多输出节点尤其）——
见#link("/user/capabilities.html")[能力速览]里实际能调到的函数。

= 原生枚举

暂不支持 `match`，只能 `==` 比较

= 事件

自定义事件、事件之间的参数传递

= 其它

`#[inline]` 的语义（内联由 rustc 的 MIR 内联器决定，写不写这个属性不影响）
