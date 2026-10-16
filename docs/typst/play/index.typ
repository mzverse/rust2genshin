#import "/lib/lib.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列,`#let title` 已经由 template 渲染成 <h1> 了,再写一遍 `= 摆弄`
// 就是同一个词连着出两行。
#show: template.with(toc: false, with_outline: false);

#let title = [摆弄];
#let order = 2;

#set raw(lang: "rust");

这一页还空着。顶栏左侧那两个分区选项卡已经能互相跳转了 —— 「文档」回到#link("/index.html")[文档主页],「摆弄」指回本页;选中哪一颗由 `header.js` 按 `location.pathname` 算,在本页或它下面就选「摆弄」,其余一律「文档」。

= 这里将来放什么

还没定。要加交互部分就按#link("/arch/docs/custom-element.html")[自定义元素]那一套走:`custom_element` 声明,shadow root 里的样式和脚本随元素一起来。

注意这一页关掉了侧栏,所以顶栏那两颗 ☰ 是死的 —— `header.js` 会看出本页既没有 `toc-component` 也没有 `nav.outline`,自动把它们藏掉,不用在这里手工标记。