#import "/lib/lib.typ": *;
#let title = [排版];
#show: template;

#set raw(lang: "css");

改这个文档站的样式时，有三个结构性原因会让 CSS 反复出问题。不是选择器写错了，是假设和 Typst 实际导出的 DOM 对不上。

= 一、块级内容被包进 `.block`

Typst 导出 HTML 时，块级内容外面会多包一层 —— `<span class="block" ...>` 或 `<div class="block container" ...>`。

所以 `X > ul`、`X > div` 这类*子代选择器*经常匹配不上，真实结构是 `X > .block > ul`。

写选择器时用 `X > .block > ul`，或者干脆用后代选择器（`X ul`）。

= 二、全局 `line-height: 0.8`

`lib/css/style.css` 里有一条全局 reset：

```css
* {
    line-height: 0.8;
}
```

它比正文字号还小（12pt 的字配 0.8 的行高），正文里的 `li` 会上下叠字。

补救方式是在 `style.css` 里给 `p` 和 `li` 各自设回 `line-height: normal`；页内目录和侧栏各自在自己的 CSS 里重设成 `1.4`。

*不要改全局那一行* —— Typst 的数学公式定位依赖它。

= 三、`.block` 的宽度

```css
.block {
    width: fit-content;
}
```

块级元素默认撑满，这里被收窄成 `fit-content`。给这类元素设宽度、`flex`、`margin: auto` 时要把这一点算进去。

= 怎么查

改之前先确认实际 DOM 是什么样，别照着源码猜：

1. 后台挂着 `cargo run -p rust2genshin-docs --bin watch`（见#link("/dev/docs/index.html")[文档]）
2. 用浏览器开发者工具，或者用 Playwright 往 shadow root 里注入一段临时 `<style>` 验证猜想
3. 确认之后再改 `lib/css/` 里的源码

这几个坑之所以难查，是因为它们*不报错* —— 样式不生效、布局错位，构建完全正常。
