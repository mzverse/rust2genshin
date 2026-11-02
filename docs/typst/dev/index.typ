#import "/lib/lib.typ": *;
#show: template;

#let title = [开发];
#let order = 1;

#set raw(lang: "rust");

这一栏讲*怎么动手改*：从哪下手、改完怎么验。理由在#link("/arch/index.html")[架构]栏。

= 想做什么

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

= 跑起来

改完怎么验、出错怎么看，见#link("/dev/build.html")[构建]与#link("/dev/debugging.html")[调试]。

两个调试工具：*viewer* 能拖入 `.ogia` / `.rlib` 看中间态 IR、逐条跑优化规则；*playground* 是文档站「摆弄」页的后端。

改站点本身先看#link("/dev/docs/index.html")[文档]，改样式前看#link("/dev/docs/layout.html")[排版]（那三个坑不报错）。
