#import "/lib/lib.typ": *;
#show: template;

#let title = [列表];
#set raw(lang: "rust");

#import "/lib/card.typ": *;

可以理解为一个在栈上的`Vec`，也可*拓展*和*收缩*

在底层和*数组*相同，也能够相互转换

= 构造

```rust
use rust2genshin_lib::list::{list, List};

let a = list![1, 2, 3];        // 字面量
let b: List<i32> = [1, 2, 3].into();   // 数组转过来，空转换
let c: List<i32> = List::default();    // 空列表
```

数组字面量能直接当列表用，因为 `[T; N]` 和 `List<T>` 在 IR 层是同一个类型。

= 读写

```rust
let mut v = list![1, 2, 3];

v[0];            // 读
v.set(0, 7);     // 写
v.push(9);       // 追加
v.insert(1, 5);  // 中间插入，后面的后移
v.remove(1);     // 移除，后面的前移
v.len();
```

#card_info[
    `v[0] = 7` 也能编译，但写元素走的是内联的「写视图」

    用 `set` 更清楚，也不依赖内联
]

= 元素类型

元素只能是节点图里有列表变体的原生类型：整数、字符串、实体、GUID、浮点、
三维向量、布尔、配置、预制件、阵营

结构体和嵌套列表会被包成单字段结构体再放进去 —— 能写，但多一层

= 析构

`List` 的 `Drop` 会逐个析构元素，节点图里体现在每个元素的析构节点上
