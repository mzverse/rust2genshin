#import "/lib/lib.typ": *;
#show: template;

#let title = [开发];
#let order = 1;

#set raw(lang: "rust");

这一栏讲*怎么动手改*：从哪下手、改完怎么验。

「为什么这么设计」在#link("/arch/index.html")[架构]栏，「怎么用」在#link("/user/index.html")[使用]栏。

= 动手

#table(
    columns: (auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*想做的事*], [*去哪*],

    [让用户在 `lib/` 里调到一个新节点],
    [#link("/dev/add-native.html")[加一个原生调用] —— 带 `#[native_calc(N)]` / `#[native_exec(N)]` 就够，不用改 core],

    [加一个编译器自己要用的节点],
    [#link("/dev/add-node.html")[加一个节点] —— 构造 `NodeKind`，再在 MIR 翻译处用它],

    [加一个事件],
    [#link("/dev/add-native.html")[加一个原生调用] 的「事件」一节 —— `#[event(N)]` 标在 struct 上],

    [加一个容器类型、改 API 命名],
    [#link("/dev/conventions.html")[约定] —— 命名规则、句柄类型的三层约束、踩过的坑],
)

= 构建与调试

#list(
    link("/dev/build.html", [构建]) + [ —— 工具链、后端怎么被 rustc 加载、端到端构建流程、CI],
    link("/dev/debugging.html", [调试]) + [ —— 诊断输出、被忽略的断言、增量构建、Python 工具],
)

= 工具

#list(
    link("/dev/viewer.html", [viewer]) + [ —— 拖入 `.ogia` / `.rlib` 浏览中间态 IR，跑优化规则看它改了什么],
    link("/dev/playground.html", [playground]) + [ —— 文档站「摆弄」页的后端，一次性 docker 容器里跑 cargo build],
)

= 文档站

#list(
    link("/dev/docs/index.html", [文档]) + [ —— Typst 环境、编译与 watch、编辑器配置],
    link("/dev/docs/layout.html", [排版]) + [ —— Typst 导出的 DOM 形状、全局行高、`.block` 宽度],
    link("/dev/docs/card.html", [卡片]),
    link("/dev/docs/typst.html", [Typst 的简单教程]),
)
