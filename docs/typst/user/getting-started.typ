#import "/lib/lib.typ": *;
#let title = [上手];
#show: template;

#set raw(lang: "rust");

= 前置

- #link("https://rustup.rs/")[rustup]（自带 cargo）
- `protoc` —— `core/build.rs` 用 `prost-build` 生成 proto 绑定的 Rust 代码；CI 里装的是 `protobuf-compiler`

不用手动装 nightly。工具链版本被仓库根的 `rust-toolchain.toml` 钉死，rustup 第一次在这个项目里跑 cargo 时会自动装它，以及它列出的三个组件（`rustc-dev`、`rust-src`、`llvm-tools-preview`）—— 后端依赖 `rustc_private`，缺一个都编译不过。

= 克隆并跑通

```shell
git clone https://github.com/mzverse/rust2genshin
cd rust2genshin
cargo run -p build-demo --bin beyond-server
```

这一条命令会依次重新编译后端 cdylib、生成目标平台描述、再用后端把 `demo` 编译成节点图。产物在：

```shell
target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia
```

`build-demo` 有 `beyond-server` 和 `classic-server` 两个 bin，对应两种 `-gamemode`，产物目录名不同。

= 写自己的节点图

节点图就是普通 Rust 代码，写在 `demo/src/lib.rs` 里（`demo` 只是一个示例目标 crate）。会被编译进节点图的函数要用属性标记：

- `#[unsafe(no_mangle)]` / `#[unsafe(export_name = "名字")]` —— 导出为复合节点
- `#[event_listener]` —— 事件监听入口

其余 `fn` 不导出，但同样会编译成独立的复合节点，只是不可独立导入。细节见#link("/user/export.html")[导出函数]和#link("/user/events.html")[事件]两页。

随手试一个：

```rust
#[unsafe(no_mangle)]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (-b + delta(a, b, c).sqrt()) / (2. * a)
}

fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}
```

改完 `demo/src/lib.rs` 直接重跑上面那条 `cargo run` 即可，`demo/build.rs` 会自动跟踪源码和后端 cdylib，不需要手动 `cargo clean`。

= 改成自己的项目

不想叫 `demo` 的话，要同步改的是两处*包名*：

1. `demo/Cargo.toml` 的 `package.name`（现在是 `rust2genshin-demo`）
2. `build-demo/src/build.rs` 里 `cargo!["build", "--release", "-p", "rust2genshin-demo", ...]` 的那个 `-p` 值

这两处必须一致，否则 `-p` 找不到包。根 `Cargo.toml` 的 `members` 里写的是*目录名*（`"demo"`），只有连目录一起改名时才需要动它。

产物文件名跟着第 1 项走。

也可以在 `demo/` 旁边新建自己的 crate 并加进 `members`，保留 `demo` 当参考。
