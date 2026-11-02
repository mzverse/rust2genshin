#import "/lib/lib.typ": *;
#let title = [导出函数];
#show: template;

#set raw(lang: "rust");

每个函数都是一个独立的*复合节点*，有自己的入参、出参和内部子图

`#[unsafe(no_mangle)]`决定的不是「成不成图」，而是能不能被*独立导入*

= 怎么导出

```rust
#[unsafe(no_mangle)]
pub fn my_composite() {
    // 这个函数会成为复合节点
}
```

或者指定导出名：

```rust
#[unsafe(export_name = "求根公式")]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (-b + delta(a, b, c).sqrt()) / (2. * a)
}
```

= 主图

*主图始终导出（如果存在），其它资产只导入被引用的*

主图是事件入口所在的图：用了`#[event_listener]`，事件节点和对应调用就会被加进主图

所以只要写了至少一个事件监听器，主图就存在

= 辅助函数

被调用的函数*同样*编译成独立的复合节点 —— 后端不做内联

区别只是没导出的图是*依赖资产*，导出的是*主资产*，所以下面的`delta`会作为`solve`引用的子图存在：

```rust
#[unsafe(no_mangle)]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (-b + delta(a, b, c).sqrt()) / (2. * a)
}

fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}
```

让函数「消失」的是*rustc的MIR内联器*，跑在后端之前

同crate的小函数在release下通常被内联掉，后端看不到独立函数体；跨crate调用（如`lib`里的）默认不被内联

所以「某个函数有没有自己的图」是rustc的内联决策，不是后端按导出与否定的

想稳定保留就用`#[inline(never)]`挡住内联器

= 入口

只有两类函数会被收集为编译入口：

- `#[unsafe(no_mangle)]` / `#[unsafe(export_name)]`
- `#[event_listener]`（宏会自动加`#[unsafe(no_mangle)]`）

一个都没有的话，编译出来是空的
