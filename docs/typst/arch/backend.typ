#import "/lib/lib.typ": *;
#show: template;

#let title = [后端为何这样接];

#set raw(lang: "rust");

`rust2genshin` 不是独立程序，而是 rustc 加载的 `Box<dyn CodegenBackend>`
（`core/src/compile/backend.rs:117` 的 `__rustc_codegen_backend`）。这一页讲
这个选择逼出了哪些别的决定。

= 为什么必须是 codegen 后端

另一种做法的想象空间是「独立程序读 `.rs`、自己解析、自己出图」。放弃了它，
理由是 MIR 里已经有一切语义：

- 所有权、借用、`Drop` 的顺序全是 rustc 算好的（`TerminatorKind::Drop` 就是
  已经决定好的析构点）
- 泛型已经单态化，`Instance` 直接带着实参
- 内联、常量折叠、死代码消除都跑在后端之前

自己解析源码就得把这些重建一遍，而重建出来的语义必然和真 Rust 有偏差。
挂成后端则是「白拿」—— 代价是要跟着 nightly 的 ABI 走（`rust-toolchain.toml`
钉死日期，换 nightly 会静默炸）。

= 为什么不产生机器码

`codegen_crate` 返回空的 `CompiledModules`，`link` 直接转发给 rustc 共享的
`link_binary`（`backend.rs:103`），产物是一个*只含 rmeta* 的 rlib —— 和
rustc 自带的 `DummyCodegenBackend` 同款行为。

这不是省事，而是「没有 LLVM 可委托」的必然结果：节点图不是可执行代码，
`.gia` 已经在前一步写完了，链接阶段没有任何东西可链接。

三条后果值得单独记住：

- `cargo check` *不会*触发导出 —— 它停在 analysis，不走 codegen
- `.gia` 是*副作用*产物，Cargo 眼里它不存在
- 依赖 proc-macro、或本身要链接成可执行文件的东西会失败，所以后端只挂在
  目标叶 crate 上

= 为什么能复用到 rustc 的内部 crate

rustup 工具链里 `rustc_*` 只发布 rmeta，代码全在 `rustc_driver` 的 dll 里。
所以本 crate 必须 `extern crate rustc_driver`，其余 `rustc_*` 的符号通过那个
已经加载进进程的 dll 解析 —— 和 miri 是同一套做法（`backend.rs` 的文件头
注释里写着）。这条不是可选技巧：少了 `extern crate rustc_driver`，链接会
找不到符号。

= 为什么用目标平台描述而不是命令行开关

两种加载方式等价（`-Zcodegen-backend=<dll>` 与 target spec 的
`default-codegen-backend`），仓库选后者，因为它把*产物文件名*一起定了：
target spec 里的 `dll-suffix` 设成 `.gia`，产物就自动是 `<crate 名>.gia`，
不需要构建脚本再搬一次文件。

这也是 `build-demo` 要生成 `genshin-unknown-beyond-server.json` 的原因 ——
spec 里要填后端 dll 的*绝对路径*，那是编译时才确定的。
