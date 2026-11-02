#import "/lib/lib.typ": *;
#show: template;

#let title = [Mut];
#let tag = <tag-mut>

#set raw(lang: "rust");

对应 Rust 中的 `&mut T`，表示对一个局部变量的引用

= 为什么是两个槽

逻辑上 `&mut T` 分 setter 和 getter 两半，编译后是 `(LocalSetter, PartialMut<T>)`
元组，扁平化成两个独立的局部变量槽：

- setter 那半只能设置*整个*变量 —— 节点图里没有「指向某处」的句柄，写入只能落到
  一个已知的局部变量上
- getter 那半给的是#link(<tag-partial-mut>)[`PartialMut<T>`]

分成两格而不是一个结构体，是因为 setter 必须在控制流上用（它是执行节点），而值那半
是纯数据。合成一个值就得给「写」伪装成纯值节点，那和节点图的语义冲突。

= 字段

字段的 `&mut` 由两半拼成：`FieldSetter` 加一次解构。

`FieldSetter<idx>` 吃 `PartialMut<Owner>`、交出 `LocalRef<字段>`；读那一侧不走它，
而是直接解构 owner 视图取第 `idx` 个出口。优化期把每一处经 `FieldSetter` 的写换成
`Modify`、把读换成同一个解构，剩下的 `FieldSetter` 降级时 panic。

必须消掉它，是因为节点图里没有「字段句柄」这种东西 —— 只能靠 `Modify` 这个执行节点
直接改字段，而 `Modify` 吃的是整个 owner。细节见#link("/arch/ir/index.html")[IR]。
