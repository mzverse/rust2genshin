#import "/lib/lib.typ": *;
#let title = [架构];
#show: template;

#set raw(lang: "rust");

这一栏是*设计本身*：这个项目分成哪几块、数据怎么流过它们、为什么这么设计。

它不回答「怎么动手改」（那是#link("/dev/index.html")[开发]栏），也不回答「怎么用」（那是#link("/user/index.html")[使用]栏）。

= 目标产物

#link("/arch/low/index.html")[节点图] —— *被编译的目标*：数据流、局部变量、循环、控制流、变量修改事件。

这些规则和 `rust2genshin` 无关 —— 直接在节点编辑器里手拼节点也一样成立。但它是整个项目的地基：不先理解这一层，就没法理解编译出来的东西为什么长那样。

= 编译器

Rust → 节点图。

#list(
    link("/arch/compile/index.html", [编译管线]) + [ —— 入口点从哪来、一个入口怎么展开、`lower`],
    link("/arch/compile/locals.html", [局部变量]) + [ —— MIR 的 `Local` / `Place` 投影怎么变成节点图的局部变量],
    link("/arch/compile/cast.html", [类型转换]) + [ —— `as` 怎么变成转换节点],
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
