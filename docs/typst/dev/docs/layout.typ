#import "/lib/lib.typ": *;
#let title = [排版];
#show: template;

#set raw(lang: "css");

改这个文档站的样式时，有四个结构性原因会让 CSS 反复出问题。不是选择器写错了，是假设和 Typst 实际导出的 DOM 对不上。

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

= 四、颜色得走 token，且 token 走的是继承

深色模式把颜色全收进了 `lib/css/theme.css`：浅色值写在 `:root`，深色值只写在 `:root[data-theme="dark"]`（`theme.css:20` 与 `theme.css:142`）这一处，其余文件一律引用 `var(--x)`。这么定不是洁癖，是因为下面四个组件各自渲染在自己的 shadow root 里：

- `toc-component`（`lib/js/toc.js:7`）
- `code-block`（`lib/js/code_block.js:16`）
- `expand-panel`（`lib/js/expand-panel.js:11`）
- `ui-graph`（`lib/js/graph.js:210` 与 `graph.js:444`）

页面上写的选择器进不去这些 shadow root。症状是：在 `lib/css/` 的某个页面样式里给目录条目、复制按钮、节点盒子加一条 `color: ...`，构建正常、开发者工具里选择器也能在 Styles 面板搜到，但*元素颜色纹丝不动* —— 因为那个元素根本不在你这条规则的树里。能穿过 shadow 边界的只有*继承的自定义属性*，所以颜色必须在 `:root` 声明一次、在任何深度用 `var()` 取用；连 `--header-h` 这种布局常量也只能住在 `theme.css`，因为两侧抽屉靠它算偏移，而抽屉在 shadow 里（`theme.css:5-8`）。

`ui-graph` 的节点盒子、连线、控件还多一层：它们的 CSS 是 `graph.js` 里内联在 shadow root 中的一段 `<style>` 字符串（`graph.js:211-213`、`graph.js:447-486`），同一套 token 两边同名引用。那里 `var()` 照样解析得到 —— 边界挡的是选择器，不是继承。代价是页面*两个方向都覆盖不了*它：既改不动节点盒子的样式，也没法把某些样式从图里摘掉。

`var()` 有个盲区：写不进 `url("data:image/svg+xml,…")`。色值直接烙进 data URI 之后就是一个主题死角。顶栏那两颗 ☰ 按钮原先就是这么画的，深色下纹丝不动；改成 `mask-image` 加 `stroke='currentColor'`（`outline.css:174`、`outline.css:140`）之后 `currentColor` 才把 token 层接进来。想给别的图标上色，先确认它走的是 `mask-image` 还是 `background-image`，后者多半改不动。

== 另一个陷阱：Typst 把颜色写成内联 `style`

`style.typ` 的 `show text` 只要 `text.fill` 不等于默认值就往 `<span>` 上写内联 `color:`（`style_text`，`lib/style.typ`）。内联样式压得过一切普通规则，token 层对它完全无效。这既是它改不动的唯一原因，也是它唯一能改的地方：`code_block.css` 末尾那十条规则用 `!important` 把内联色值按回 token，认的是*一个色值 → 一个角色*：

```css
:root[data-theme="dark"] code span[style*="#74747c"] {
    color: var(--code-syn-comment) !important;
}
```

不要图省事写成 `span[style*="color"]` 一条通配 —— 那会把十种 token 色刷成同一个颜色，整个代码块变成单色，高亮全毁。别名的症状：*代码块看着「有高亮」，但所有 token 一样颜色*。

== 深色是脚本定的，不是媒体查询

`theme.js` 作为 `<head>` 里的阻塞脚本（`template.typ:40`）在第一帧之前把 `data-theme` 写到 `<html>` 上，偏好存在 `localStorage`，被禁用时退回 `matchMedia`。代价是深色模式依赖 JS：把脚本关掉，页面停在浅色，`data-theme` 不存在，`:root[data-theme="dark"]` 整块不生效。这个取舍是有意的 —— 侧栏、目录抽屉、节点图、复制按钮本来就全是 JS 驱动的（`theme.js` 的文件头）。

= 怎么查

改之前先确认实际 DOM 是什么样，别照着源码猜：

1. 后台挂着 `cargo run -p rust2genshin-docs --bin watch`（见#link("/dev/docs/index.html")[文档]）
2. 用浏览器开发者工具，或者用 Playwright 往 shadow root 里注入一段临时 `<style>` 验证猜想
3. 确认之后再改 `lib/css/` 里的源码
4. 颜色不生效时先分清是哪一类：元素在 shadow root 里（选择器到不了，换 `var()`）、色值烙在 data URI 里（换 `mask-image` + `currentColor`）、还是被 Typst 写成了内联 `style`（只能 `!important` 逐个色值覆盖）。在开发者工具的 Styles 面板里看那条属性是命中了哪条规则，就知道该往哪个方向查。

这几个坑之所以难查，是因为它们*不报错* —— 样式不生效、布局错位，构建完全正常。
