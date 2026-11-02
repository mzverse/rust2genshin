#import "/lib/lib.typ": *;
#let title = [架构];
#show: template;

#set raw(lang: "rust");

这一栏讲*为什么是这样*：每处架构做了哪些取舍、放弃了什么、代价落在哪。

它不回答「怎么动手改」（那是#link("/dev/index.html")[开发]栏），也不回答「怎么用」（那是#link("/user/index.html")[使用]栏）。

= 目标产物

#link("/arch/low/index.html")[节点图] —— *被编译的目标*：数据流、局部变量、循环、控制流、变量修改事件。

这些规则和 `rust2genshin` 无关 —— 直接在节点编辑器里手拼节点也一样成立。但它是整个项目的地基：不先理解这一层，就没法理解编译出来的东西为什么长那样。

= 编译器

Rust → 节点图。

#list(
    link("/arch/backend.html", [后端为何这样接]) + [ —— 为什么挂成 codegen 后端、为什么不产生机器码、rustc 内部 crate 怎么复用到],
    link("/arch/compile/index.html", [编译管线]) + [ —— 入口点从哪来、一个入口怎么展开、`lower`],
    link("/arch/compile/locals.html", [局部变量]) + [ —— 为什么复合类型必须被打散成标量槽],
    link("/arch/compile/loops.html", [循环改写]) + [ —— IR 里的控制流环怎么变成有限循环节点、计数循环怎么认出来],
    link("/arch/compile/cast.html", [类型转换]) + [ —— `as` 怎么变成转换节点],
    link("/arch/calling-convention.html", [调用约定]) + [ —— 复合节点的数据流形态，以及纯度问题为什么还没解决],
)

= 中间表示

`IrKind` / `IrNodeId` 存在的理由，以及它们怎么在 `lower` 里被消灭。

#list(
    link("/arch/ir/index.html", [IR]) + [ —— 类型与节点的中间形态、读值与借用在图上的区别],
    link("/arch/ir/kind/index.html", [类型]) + [ —— `IrKind` 的各个变体],
    link("/arch/ir/node/local.html", [节点]) + [ —— `Local` / `SetLocal` / `switch` / `nop` / `unreachable`],
)

= 产物与工具

#list(
    link("/arch/asset.html", [资产格式]) + [ —— `.gia` 的字节布局、proto 字段、值的编码],
    link("/arch/viewer.html", [viewer]) + [ —— 中间态浏览器：布局算法与优化面板的设计取舍],
    link("/arch/play.html", [摆弄页]) + [ —— 浏览器、playground 服务、docker 三层的组件与数据流],
)

= 外部依据

#link("/arch/reference.html")[参考项目] —— 节点 ID、proto 字段、`.gia` 格式的外部依据，以及它和本项目的差异。

#link("/arch/docs/index.html")[文档站] —— 本站自身怎么从 Typst 编译并发布，以及文档站里那些自定义元素怎么搭起来的。
