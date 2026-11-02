#import "/lib/lib.typ": *;
#let title = [项目定位];
#show: template;

#set raw(lang: "rust");

`rust2genshin`是一个*非官方*的rustc codegen后端：把Rust代码编译成原神千星奇域的节点图资产`.gia`

它以`Box<dyn CodegenBackend>`的形式被`rustc -Zcodegen-backend`加载，*不产生机器码* —— 一个`fn`就是一个复合节点，函数体里的每条语句、每个运算变成节点和连线

= 是什么，不是什么

- 把Rust编译成节点图，不是反向从节点图生成Rust
- 与米哈游官方无关联
- 不是通用后端 —— 只适合「整个crate就是一张节点图」的程序
- 只支持*服务器*节点图

= 产物

一次编译产出一个`.gia`，导入后成为一张节点图资产

目标平台由仓库根的`genshin-unknown-beyond-server.json`描述：`dll-suffix`设成`.gia`，所以产物是crate名加后缀（`rust2genshin-demo` → `rust2genshin_demo.gia`）

除节点图本身，编译过程不生成任何可执行代码 —— 链接阶段返回空产物
