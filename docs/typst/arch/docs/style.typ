#import "/lib/lib.typ": *;
#show: template;

#let title = [样式];

#set raw(lang: "html");

= 段落

编译为`<par>`而不是`<p>`

因为`<p>`遇到默认块级标签（无视style）会自动关闭，包括嵌套里的元素

= 容器

#[
    #set raw(lang: "typst");
    包括`box`、`block`、`stack`
]

容器使用```css .container```标记

```typ box```编译为`<span>`，```typ block```和```typ stack```编译为`<div>`

其中```typ stack```使用```css display: flex```，其余用默认
