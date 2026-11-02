#import "/lib/lib.typ": *;
#let title = [不被支持的特性];
#show: template;

#set raw(lang: "rust");

编译不过或明确不会支持的写法，每条附原因与替代

= `i8` / `i16`

节点图只有`i32`一种整数

```rust
let a: i8 = 1;    // 不行
let a: i32 = 1;   // 统一用 i32
```

= 非 `i32` 的枚举表示

原生枚举只有`#[repr(i32)]`

```rust
#[repr(u32)]    // 不会被当作原生枚举
#[repr(i32)]    // 一律用这个
enum Kind { A = 1 }
```

= 递归

节点图的控制流不允许成环，递归天然需要环

```rust
// 不行：自己调自己
fn fact(n: i32) -> i32 {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}
```

替代是*协程* —— 见#link("/user/async.html")[异步]

= 元组的 `==`

`p == q`被lower成`PartialEq::eq(...)`的trait调用，后端不建模trait dispatch

```rust
let same = p == q;                        // 不行
let same = p.0 == q.0 && p.1 == q.1;      // 逐字段比
```

详见#link("/arch/compile/locals.html")[局部变量]

= trait object

虚表在节点图里开销过大，编译期也无法解析到具体函数

尽量用泛型 + 单态化替代
