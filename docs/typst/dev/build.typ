#import "/lib/lib.typ": *;
#let title = [构建];
#show: template;

#set raw(lang: "rust");

= 工具链

`rust-toolchain.toml` 把工具链钉死：

```toml
[toolchain]
channel = "nightly-2026-10-08"
components = ["rustc-dev", "rust-src", "llvm-tools-preview"]
```

工具链必须是 nightly（要用 `#![feature(rustc_private)]`）。组件里 `rustc-dev` 提供 `rustc_*` crate，`rust-src` 是 `-Zbuild-std=core` 需要的标准库源码。

rustup 第一次在这个目录里跑 cargo 时会自动装齐，不用手动 `rustup component add`。

= 后端是编译进程内的插件

`rust2genshin` 不是独立程序，而是被 rustc 加载的 `Box<dyn CodegenBackend>`。

rustc 通过 `__rustc_codegen_backend` 符号拿到它（`core/src/compile/backend.rs` 的末尾），在 analysis 之后调用 `codegen_crate` —— 那时 HIR 已经可用，导出 `.gia` 就在这一步做。

依赖能成立，靠的是 dylib 符号复用：rustup 工具链里 `rustc_*` 只发布 rmeta，代码本身都在 `rustc_driver` 的 dll 里。所以入口必须 `extern crate rustc_driver`，其余 `rustc_*` 的符号通过那个已经加载进进程的 dll 解析。这和 miri 的做法一样，`core/src/compile/backend.rs` 的文件头注释里写着。

= 后端不产生机器码

`codegen_crate` 导出完 `.gia` 就返回*空的* `CompiledModules`；`link` 阶段走 rustc 共享的 `link_binary`，产物是仅含 rmeta 的 rlib —— 与 rustc 自带的 `DummyCodegenBackend` 同款行为。

这带来三条要记住的后果：

- `cargo check` *不会*调用后端，只有走到 codegen 的 `cargo build` 才会触发导出
- `.gia` 是*副作用*产物，不是 Cargo 眼里的主产物
- 依赖 proc-macro、或本身要链接成可执行文件的 crate 会失败 —— 所以只对目标叶 crate 挂后端，依赖照常由 LLVM 编译

= 怎么被加载

两种方式，效果一样：

- 命令行：`rustc -Zcodegen-backend=<后端 dll 路径>`
- 目标平台描述：target spec 里的 `"default-codegen-backend": "<路径>"`

仓库里走的是第二种。`genshin-unknown-beyond-server.json.template` 是关键字段：

```json
{
  "dll-prefix": "",
  "dll-suffix": ".gia",
  "linker": "./target/debug/rust2genshin-linker",
  "pre-link-args": { "gcc": ["-gamemode", "beyond"] },
  "default-codegen-backend": {backend}
}
```

`{backend}` 是占位符，由 `build-demo` 填成后端 dll 的*绝对路径*再写出 `.json`。

注意 `dll-suffix` 是 `.gia` —— 所以产物文件名就是 crate 名加 `.gia`（横线换成下划线）。

= 构建流程

```shell
cargo run -p build-demo --bin beyond-server
```

这一个命令做三件事：

1. `cargo build -p rust2genshin` —— 重编译后端 cdylib，顺带编出 `rust2genshin-linker`（`core/src/linker.rs`，就是上面 target spec 里 `linker` 指向的那个 bin）
2. 读 `.json.template`，把 `{backend}` 换成后端 dll 的绝对路径，写出 `genshin-unknown-beyond-server.json`
3. `cargo build --release -p rust2genshin-demo --target ./genshin-unknown-beyond-server.json -Zunstable-options -Zjson-target-spec -Zbuild-std=core`

产物：

```shell
target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia
```

`build-demo` 有两个 bin：`beyond-server` 和 `classic-server`。它们只有 `-gamemode` 和产物目录名不同，所以 `cargo run -p build-demo` 是*不合法*的（cargo 无法从两个 bin 里挑一个），必须带 `--bin`。

= 依赖：proto

`core/proto/asset.proto` 定义了 `.gia` 里 protobuf 那一层的结构。

`core/build.rs` 在编译期用 `prost-build` 把它生成 Rust 绑定到 `$OUT_DIR/rust2genshin.rs`，然后在 `core/src/asset.rs` 里 `include!` 进来。

改了 `.proto` 要重新构建（`prost-build` 会跟踪文件变化）。Python 侧不用重新生成 —— `tools/gia.py` 在运行时读 proto 文件。

= CI

`.github/workflows/build.yml`：Ubuntu 上 `sudo apt install protobuf-compiler`，跑 `cargo run -p build-demo --bin beyond-server`，上传 `target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia`。

`viewer` 不在 workspace 的 `default-members` 里。它依赖 gpui，Linux 上要 X11/Wayland 和字体库，CI 没装 —— 要构建它得显式 `cargo build -p rust2genshin-viewer`。

文档站有单独的一条 workflow（`.github/workflows/docs.yml`），见#link("/arch/docs/index.html")[文档站]。
