#import "/lib/lib.typ": *;
#let title = [不被支持的特性];
#show: template;

#set raw(lang: "rust");

编译不过或明确不会支持的写法，每条附原因与替代

= 整数与浮点的宽度

节点图只有 `i32` 和 `f32` 两种数值类型。`isize` / `i32` / `u32` / `usize` 都映射到 `i32`，
`f32` 保持不变；其余宽度在编译期报错：

```rust
let a: i8 = 1;     // 不行
let a: i64 = 1;    // 不行
let a: u8 = 1;     // 不行
let a: f64 = 1.;   // 不行

let a: i32 = 1;    // 统一用 i32
let a: f32 = 1.;   // 统一用 f32
```

节点图里的整数运算没有溢出检查，`u32` 的「一定非负」在这条路上也不成立 ——
当成 `i32` 用即可。

= 非 `i32` 的枚举表示

原生枚举只有 `#[repr(i32)]`

```rust
#[repr(u32)]       // 不会被当作原生枚举
#[repr(i32)]       // 一律用这个
enum Kind { A = 1 }
```

= 递归

节点图的控制流不允许成环，而递归天然需要环

```rust
// 不行：自己调自己
fn fact(n: i32) -> i32 {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}
```

替代是*协程* —— 见#link("/user/async.html")[异步]

= 元组的 `==`

`p == q` 被 lower 成 `PartialEq::eq(...)` 的 trait 调用，而后端不建模 trait dispatch

```rust
let same = p == q;                        // 不行
let same = p.0 == q.0 && p.1 == q.1;      // 逐字段比
```

= trait object

虚表在节点图里开销过大，编译期也无法解析到具体函数

用泛型 + 单态化替代

= `Box` 的写

`Box<T>` 的借用是*假借用* —— 写元素走的是「写视图」（`list_set` / `Modify`），
节点图里没有写句柄

于是能读、能借字段、能调 `&mut self` 的方法，但*不能整体赋值*：

```rust
let b = Box::new(1);
*b = 2;                       // 不行：要真写句柄
Box::into_inner(b)            // 要换值就先取出来
```

`Box<结构体>` 的字段写（`m.x = 114`）同理 —— 见#link("/user/heap.html")[堆分配]
