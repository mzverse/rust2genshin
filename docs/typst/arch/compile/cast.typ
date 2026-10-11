#import "/lib/lib.typ": *;
#let title = [类型转换];
#show: template;

#set raw(lang: "rust");

`as` 在 MIR 里是 `Rvalue::Cast`，翻译在 `core/src/compile/func.rs:239`。

= 三条路

1. *源和目标同类型* → 直通，不建节点（`i32 as isize` 之类）
2. *在支持表里* → 建一个转换节点
3. *否则* → `span_err`，报出 `CastKind` 和源/目标类型

源和目标类型各自先过 `compile_ty`。所以 `f64`、`u32` 这些 `compile_ty` 不认的类型在*更早*就报错了，根本走不到第 3 条。

= 转换节点

`node_cast(from, to) -> Option<NodeKind>`（`core/src/node/arithmetic.rs:588`）：shell 固定 ID 180，kernel 由 `(K, V)` 组合决定：

#table(columns: 4, align: center + horizon)[(K → V)][kernel][(K → V)][kernel
][`Int → Bool`][180][`Bool → Int`][185
][`Int → Float`][181][`Bool → String`][186
][`Int → String`][182][`Float → Int`][187
][`Entity → String`][183][`Float → String`][188
][`Guid → String`][184][`Vec3 → String`][189
]

表外的组合返回 `None`。输入引脚是 `R<K>`（多态），还要按源类型设 selected index（`Int`→0、`Entity`→1、`Guid`→2、`Bool`→3、`Float`→4、`Vec3`→5）。

要加新组合，在 `node_cast` 的 match 里添一条（kernel id）和 `selected_in` 的对应项 —— 两处都要，漏一处会在运行时给出错的多态引脚。
