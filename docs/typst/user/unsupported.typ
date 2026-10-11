#import "/lib/lib.typ": *;
#let title = [不被支持的特性];
#show: template;

#set raw(lang: "rust");

这些是*编译不过*或明确不会支持的写法。每条附上原因和替代做法。

= `i8` / `i16`

节点图里的整数节点只有 `i32` 一种，没有更窄的整型。

```rust
let a: i8 = 1;    // 不行
let a: i32 = 1;   // 统一用 i32
```

= `#[repr(u32)]` 等非 `i32` 的枚举表示

原生枚举只有 `#[repr(i32)]` 一种表示。

```rust
#[repr(u32)]    // 不会被当作原生枚举
#[repr(i32)]    // 一律用这个
enum Kind { A = 1 }
```

= 递归调用

节点图的控制流不允许成环 —— 反复执行的代码必须靠原生的循环节点「重新进入」，不能跳回自己。递归天然需要环。

```rust
// 不行：自己调自己
fn fact(n: i32) -> i32 {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}
```

反复执行的替代是*协程*：函数标 `#[asynchronous]`，循环标 `#[async_loop]`，每轮循环让出一帧，才成为合法的控制流。注意裸 `while` 不带 `#[async_loop]` 不算 —— 它会在一次执行里跑完，仍然没有环。

```rust
#[asynchronous]
pub fn count_up(n: i32) {
    let mut i = 0;
    #[async_loop]
    while i <= n {
        log(i);
        i += 1;
    }
}
```

此时调用方拿到的不是返回值而是协程 —— 完整用法见#link("/user/async.html")[异步]。

= 元组的 `==`

rustc 把 `p == q` lower 成 `<(A, B) as PartialEq>::eq(...)` 的 trait 调用，而后端不建模 trait dispatch。

```rust
let same = p == q;                        // 不行
let same = p.0 == q.0 && p.1 == q.1;      // 逐字段比
```

详见#link("/arch/compile/locals.html")[局部变量]。

= trait object（`dyn`）

虚表在节点图里开销过大，编译期也无法解析到具体函数。

*状态*：可能不予支持。尽量用泛型 + 单态化替代。
