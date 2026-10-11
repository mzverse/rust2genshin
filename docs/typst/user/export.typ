#import "/lib/lib.typ": *;
#let title = [导出函数];
#show: template;

#set raw(lang: "rust");

函数在节点图里都是一个独立的*复合节点*，有自己的入参、出参和内部子图。`#[unsafe(no_mangle)]` 决定的不是「成不成图」，而是它能不能被*独立导入*。

= 怎么导出

用 `#[unsafe(no_mangle)]`，或者用 `#[unsafe(export_name = "自定义名字")]` 指定导出名：

```rust
#[unsafe(no_mangle)]
pub fn my_composite() {
    // 这个函数会成为复合节点
}
```

```rust
#[unsafe(export_name = "求根公式")]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (-b + delta(a, b, c).sqrt()) / (2. * a)
}
```

导出后，这个函数在节点图里是一个*复合节点*：有自己的入参、出参，以及一个内部子图。

= 主图

*主图始终导出（如果存在）。未导出的、也没有被任何导出资产引用的资产，无法被导入。*

主图是事件入口所在的图：用了 `#[event_listener]`，事件节点和对应的调用就会被加进主图。所以只要写了至少一个事件监听器，主图就存在，不需要额外标注。

= 辅助函数

被别的函数调用的函数*同样*会编译成独立的复合节点 —— 后端不做内联，调用点在图上就是一个指向它的复合节点。区别只是：没导出的图是*依赖资产*（DEP），导出的才是*主资产*（PRIMARY）。所以下面这个 `delta` 会作为 `solve` 引用的一个独立子图存在，而不是被塞进 `solve` 里：

```rust
#[unsafe(no_mangle)]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (-b + delta(a, b, c).sqrt()) / (2. * a)
}

fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}
```

会让一个函数「消失」的是 *rustc 自己的 MIR 内联器*，它跑在后端之前。同一个 crate 里的小函数在 release 下通常被内联掉，后端根本看不到独立的函数体，自然也就没有对应的复合节点；跨 crate 调用（比如 `lib` 里的）默认不被内联，会保留成复合节点。所以「某个辅助函数有没有自己的一张图」是 rustc 的内联决策，不是后端按「导出与否」定的规则。

想让一个辅助函数*稳定*地成为独立复合节点，加 `#[inline(never)]` 挡住内联器。

= 入口

会被当作编译入口收集的函数，只有这两类：

- `#[unsafe(no_mangle)]` / `#[unsafe(export_name)]` 标记的函数
- `#[event_listener]` 标记的函数（宏会自动给它加上 `#[unsafe(no_mangle)]`）

一个都没有的话，编译出来是空的。
