#import "/lib/lib.typ": *;
#let title = [项目定位];
#show: template;

#set raw(lang: "rust");

`rust2genshin` 是一个*非官方*的 rustc codegen 后端：把目标 crate 的 Rust 代码编译成原神千星奇域的节点图资产文件 `.gia`。

它以 `Box<dyn CodegenBackend>` 的形式被 `rustc -Zcodegen-backend` 加载。和普通的 codegen 后端不同，它*不产生机器码* —— 一个 Rust `fn` 会被翻译成节点图里的一个复合节点，函数体里的每条语句、每个运算变成节点和连线。

= 它是什么、不是什么

- 是把 Rust 编译成节点图，不是把节点图反向生成 Rust
- 与米哈游官方无关联
- 不是通用后端 —— 只有「整个 crate 就是一张节点图」这种程序才适合它
- 目前只支持*服务器*节点图；客户端节点图尚未实现

= 产物

一次编译产出一个 `.gia` 文件，导入后成为一张节点图资产。

目标平台由仓库根目录的 `genshin-unknown-beyond-server.json` 描述：`dll-suffix` 被设成 `.gia`，所以产物文件名就是 crate 名加 `.gia` 后缀（`rust2genshin-demo` → `rust2genshin_demo.gia`）。

除了节点图本身，编译过程不生成任何可执行代码 —— 链接阶段返回空的产物。
