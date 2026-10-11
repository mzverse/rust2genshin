#import "/lib/lib.typ": *;
#let title = [目标产物（节点图）];
#show: template;

#set raw(lang: "rust");

这一层讲的全是*目标平台本身*的规则 —— 数据流什么时候重新求值、局部变量存在哪、循环怎么重复执行、控制流为什么不能成环。这些和 `rust2genshin` 无关：直接在节点编辑器里手拼节点，规则一样成立。

#list(
    link("/arch/low/dataflow.html", [数据流与所有权]) + [ —— 出参什么时候被重新计算、数据什么时候被克隆],
    link("/arch/low/locals.html", [获取局部变量]) + [ —— 为什么不该把「初始值」接到别的节点],
    link("/arch/low/loops.html", [循环]) + [ —— 有限循环与列表迭代循环各储存什么],
    link("/arch/low/controlflow.html", [控制流]) + [ —— 为什么不允许成环，「退出循环」到底是什么],
    link("/arch/low/mutation-event.html", [变量修改事件]) + [ —— 什么时候会生成事件并入队],
)

= 再往上

*`rust2genshin` 怎么把 Rust 映射到这些节点*、*产物 `.gia` 的字节格式*，是项目自己那一层的事，在#link("/arch/index.html")[架构]顶层写。
