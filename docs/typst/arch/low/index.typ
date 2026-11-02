#import "/lib/lib.typ": *;
#let title = [目标产物（节点图）];
#show: template;

#set raw(lang: "rust");

这一层讲的全是*目标平台本身*的规则 —— 数据流什么时候重新求值、局部变量存在哪、循环怎么重复执行、控制流为什么不能成环。

这些和 `rust2genshin` 无关：直接在节点编辑器里手拼节点，规则一样成立。但它是整个项目的地基 —— 不先理解这一层，就看不出编译结果为什么长那样。

五页里最先该读的是#link("/arch/low/dataflow.html")[数据流与所有权]和#link("/arch/low/controlflow.html")[控制流]：前者决定了所有求值时机，后者解释了为什么递归不成立。

`rust2genshin` 怎么把 Rust 映射到这些节点、产物 `.gia` 的字节格式，是项目自己那一层的事，在#link("/arch/index.html")[架构]顶层写。
