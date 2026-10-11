# 简介

**非官方**的 `rustc` codegen 后端:把目标 crate 的 Rust 代码编译成原神千星奇域的节点图资产文件 `.gia`。

目前仅支持服务器节点图。

📖 **完整文档:<https://mzverse.github.io/rust2genshin/>**

> [!NOTE]
> 本文档正在迁往文档站。下面残留的章节（兼容性、关于节点图等）会逐步移走,迁移期间以文档站为准。

# Usage

## Demo

### 监听选项卡选中事件

```rust
#[event_listener]
pub fn on_tab_selected(event: TabSelectedEvent) {
    log(event.guid); // 打印Guid
    event.entity.delete(); // 删除本实体
}
```

### 简单的求根公式

```rust
#[unsafe(no_mangle)] // 导出此函数为复合节点
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (- b + delta(a, b, c).sqrt()) / (2. * a)
}

fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}
```

## Build

需要 [rustup](https://rustup.rs/)、cargo 和 [protoc](https://github.com/protocolbuffers/protobuf/releases/latest)。nightly 工具链由 `rust-toolchain.toml` 钉死，rustup 首次运行时会连同三个组件一起装好。

```shell
git clone https://github.com/mzverse/rust2genshin
cd rust2genshin
cargo run -p build-demo --bin beyond-server
```

产物是 `target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia`。

节点图写在 `demo/src/lib.rs`（`demo` 只是示例目标 crate）。改包名、换成自己的 crate 等步骤见[上手](https://mzverse.github.io/rust2genshin/user/getting-started.html)。

## Code

写法细节（导出、事件、内置事件清单）见文档站：

- [导出函数](https://mzverse.github.io/rust2genshin/user/export.html) —— 哪些 `fn` 会成为复合节点，主图是什么
- [事件](https://mzverse.github.io/rust2genshin/user/events.html) —— `#[event_listener]` 用法与内置事件清单

# 兼容性与限制

- [兼容性与限制](https://mzverse.github.io/rust2genshin/user/compatibility.html) —— 运算溢出、整数除法、扁平化局部变量、内部可变性、可变借用
- [不被支持的特性](https://mzverse.github.io/rust2genshin/user/unsupported.html) —— `i8`/`i16`、`#[repr(u32)]`、递归调用、`dyn`
- [待办](https://mzverse.github.io/rust2genshin/user/limitations.html) —— 还没做的语言特性、类型、节点、事件

# 关于节点图

节点图的语义（数据流、循环、控制流、变量修改事件）是「为什么代码会编译成这样」的那一层，已迁入架构栏：

- [数据流与所有权](https://mzverse.github.io/rust2genshin/arch/low/dataflow.html) —— 出参何时重新计算、数据何时克隆、默认值规则
- [获取局部变量](https://mzverse.github.io/rust2genshin/arch/low/locals.html) —— 为什么不该把「初始值」接到别的节点
- [循环](https://mzverse.github.io/rust2genshin/arch/low/loops.html) —— 有限循环与列表迭代循环各储存什么
- [控制流](https://mzverse.github.io/rust2genshin/arch/low/controlflow.html) —— 为什么不允许成环、「退出循环」的本质
- [变量修改事件](https://mzverse.github.io/rust2genshin/arch/low/mutation-event.html) —— 「触发事件」为 true 时发生了什么
