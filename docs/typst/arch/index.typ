#import "/lib/lib.typ": *;
#let title = [架构];
#show: template;

#set raw(lang: "rust");

这一栏讲*为什么是这样*：每处架构做了哪些取舍、放弃了什么、代价落在哪。

不回答「怎么动手改」（那是#link("/dev/index.html")[开发]栏），也不回答「怎么用」（那是#link("/user/index.html")[使用]栏）。

= 值得先读的三页

#list(
    link("/arch/backend.html", [后端为何这样接]) + [ —— 为什么是 codegen 后端、为什么不产生机器码],
    link("/arch/compile/index.html", [编译管线]) + [ —— 入口点从哪来、`lower` 为什么分那么多步],
    link("/arch/ir/index.html", [IR]) + [ —— 中间形态为什么不能省，读和写在图上怎么区分],
)

= 地基

#link("/arch/low/index.html")[节点图] —— *被编译的目标*本身的规则。这些和 `rust2genshin` 无关，直接在节点编辑器里手拼节点也一样成立，但不先理解这一层，就看不出编译结果为什么长那样。

= 产物与外部依据

#link("/arch/asset.html")[资产格式] · #link("/arch/reference.html")[参考项目] —— `.gia` 怎么编码，以及节点 ID / proto 字段的依据在哪。
