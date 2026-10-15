#import "/lib/lib.typ": *;
#show: template;

#let title = [开发];
#let order = 1;

#set raw(lang: "rust");

这一栏讲 *怎么继续开发 `rust2genshin`* —— 现在要动手改点东西，从哪下手。

和#link("/arch/index.html")[架构]栏的区别：架构讲这个项目*是怎么搭起来的*（分层、管线、格式），读它是为了理解；这一栏讲*怎么改它*（构建、调试、加节点），读它是为了动手。

= 后端

#list(
    link("/dev/build.html", [构建]) + [ —— 工具链、后端怎么被 rustc 加载、端到端构建流程、CI],
    link("/dev/debugging.html", [调试]) + [ —— 诊断输出、被忽略的断言、增量构建、Python 工具],
    link("/dev/add-node.html", [加一个节点]) + [ —— 构造 `NodeKind`，并在 MIR 翻译处用它],
    link("/dev/add-native.html", [加一个原生调用]) + [ —— 让 `lib/` 里一个函数对应到一个节点],
    link("/dev/conventions.html", [约定]) + [ —— 文档注释、命名、句柄类型的实现约束、已踩的坑],
)

= 工具

#list(
    link("/dev/viewer.html", [viewer]) + [ —— 拖入 `.ogia` / `.rlib` 浏览中间态 IR，跑优化规则看它改了什么],
)

= 文档站

改这个文档站本身：

#list(
    link("/dev/docs/index.html", [文档]) + [ —— Typst 环境、编译与 watch、编辑器配置],
    link("/dev/docs/layout.html", [排版]) + [ —— Typst 导出的 DOM 形状、全局行高、`.block` 宽度],
    link("/dev/docs/card.html", [卡片]),
    link("/dev/docs/typst.html", [Typst 的简单教程]),
)
