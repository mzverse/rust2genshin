#import "/lib/lib.typ": *;
#let title = [构建];
#show: template;

#set raw(lang: "rust");

#import "/lib/card.typ": *;

= 一条命令

```shell
cargo run -p build-demo --bin beyond-server
```

它依次做三件事：

1. `cargo build -p rust2genshin` —— 重编译后端 cdylib，顺带编出 `rust2genshin-linker`
2. 读 `genshin-unknown-beyond-server.json.template`，把 `{backend}` 换成后端 dll 的绝对路径，写出 `.json`
3. `cargo build --release -p rust2genshin-demo --target ./genshin-unknown-beyond-server.json -Zunstable-options -Zjson-target-spec -Zbuild-std=core`

产物：

```shell
target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia
```

路径是 `<target 名>/release/<crate 名>.gia`。

#card_attention[
    `build-demo` 有两个 bin：`beyond-server` 和 `classic-server`

    必须带 `--bin`，否则 cargo 无法从两个里挑一个，直接报错
]

两个 bin 只有 `-gamemode` 和产物目录名不同。

为什么要生成 target spec、为什么产物是 `.gia` —— 见#link("/arch/backend.html")[后端为何这样接]。

= 只改后端

```shell
cargo build -p rust2genshin
```

但注意：后端 cdylib 更新了，`demo` 的增量缓存未必失效。`demo/build.rs` 显式声明了对 cdylib 的 `rerun-if-changed`，所以走 `build-demo` 那条命令是安全的。

= 工具链

`rust-toolchain.toml` 把版本钉死：

```toml
[toolchain]
channel = "nightly-2026-10-08"
components = ["rustc-dev", "rust-src", "llvm-tools-preview"]
```

必须 nightly（要用 `#![feature(rustc_private)]`）。`rustc-dev` 提供 `rustc_*` crate，
`rust-src` 是 `-Zbuild-std=core` 需要的标准库源码。

rustup 第一次在这个目录里跑 cargo 时会自动装齐，不用手动 `rustup component add`。

*不要放宽这个 pin* —— 换一个 nightly，codegen ABI 会静默不匹配。

= 依赖：proto

`core/proto/asset.proto` 定义 `.gia` 里 protobuf 那一层。`core/build.rs` 用
`prost-build` 生成绑定到 `$OUT_DIR/rust2genshin.rs`，`core/src/asset.rs` 里
`include!` 进来。

改了 `.proto` 重新 `cargo build` 即可（`prost-build` 跟踪文件变化）。Python 侧
不用重新生成 —— `tools/gia.py` 在运行时读 proto 文件。

= 测试

```shell
cargo test -p rust2genshin
cargo test -p rust2genshin-playground
```

`viewer` 和 `playground` 不在 `default-members` 里，`cargo test` 不会覆盖它们，
要测就显式带 `-p`。

= CI

`.github/workflows/build.yml`：Ubuntu 上 `sudo apt install protobuf-compiler`，
跑 `cargo run -p build-demo --bin beyond-server`，上传产出的 `.gia`。

`viewer` 依赖 gpui，Linux 上要 X11 / Wayland / 字体库，CI 没装 —— 所以它被排除在
`default-members` 之外（原因见#link("/arch/viewer.html")[viewer 的设计]）。

文档站有单独一条 workflow（`.github/workflows/docs.yml`），见#link("/arch/docs/index.html")[文档站]。
