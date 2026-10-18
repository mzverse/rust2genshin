#import "/lib/lib.typ": *;
#let title = [自定义元素];
#show: template;

#set raw(lang: "rust");

文档站里有几样东西在 Typst 里没有对应物 —— 折叠面板、卡片、可拖拽缩放的节点图、抽屉侧栏。它们都靠浏览器端的 web component：Typst 负责把数据写进 HTML，剩下的交给 JS。这一页讲所有这类元素的*共同*结构，以及为此付出的几笔代价。

某一类元素自己的细节，比如节点图的 `<svg>` 层和 `<foreignObject>`，见#link("/arch/docs/graph.html")[节点图]。

= 两类增强：只加样式的，和真造 DOM 的

不是所有增强都要 JS：

- *纯样式*：`code_block`、`math_render` 这类，`import_script` + `import_style` 挂上去就完事，JS 只读写已有的元素。`custom_element` 用不上。
- *自定义元素*：`ui-card` / `ui-graph` / `expand-panel` / `toc-component` / `site-header`，`customElements.define` 出一种新标签，构造时 `attachShadow` 造出 shadow root，里面的 DOM 全是 JS 建的。

本页只讲第二种。

= 一个壳，两份内容

自定义元素由 `lib/lib.typ` 里的 `custom_element` 包出来：

```typ
#let custom_element(register, ele) = {
    [#metadata(register) <custom-element>]
    ele
}
```

它发两样东西：

1. 一条 `<custom-element>` 标签的 metadata，值是那个注册函数（下面再说它干嘛）
2. `ele` 本身 —— 真正要出现在正文里的 HTML 元素

注册函数不是直接调用的，而是被*收集*起来统一执行：

```typ
for x in query(<custom-element>).dedup() {
    (x.value)();
}
```

`template`（`lib/template.typ`）在每页的 `<head>` 里跑这一句。于是注册的内容落进 `<head>`，注册函数本身不参与正文排版。`.dedup()` 是关键：同一页里可以有几十个 `<ui-graph-node>`，但它们的注册内容一字不差，去重后只剩一份。

`ui-graph` 和 `ui-graph-node` 特意注册成*同一份*内容（`lib/graph.typ` 的 `graph_element` 两个元素共用）—— 这样同一页里图和单独摆的节点混着用，脚本也只引一次。

= 三层结构

一次 `ui-card` 展开是这样：

- *light DOM*：Typst 生成的 `<ui-card type="info">正文</ui-card>`。数据载体，也是页面里唯一真实存在的东西。
- *`<template>`*：注册函数里 `html_elem("template", id: "card")[...]` 声明的那棵影子 DOM 的原型。在 `<head>` 里，不显示。
- *shadow root*：JS 构造时 `this.shadowRoot.appendChild(template.content.cloneNode(true))`，把 `<template>` 克隆一份塞进去。每个实例一份。

所以 `lib/card.typ` 拆成两半：Typst 侧声明 `<template>` 的结构和属性名，JS 侧只管克隆和填事件：

```js
const template = document.getElementById("card");
customElements.define("ui-card", class extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
        this.shadowRoot.appendChild(template.content.cloneNode(true));
    }
});
```

卡片里的标题栏和正文框靠 `part` 属性认领（`querySelector('[part="header"]')`），内容靠命名 `<slot name="trigger">` 投影。`part` 也能从页面外用 `::part()` 改样式 —— 和 `::slotted()` 一样，都是穿透 shadow 边界的官方口子。

#set raw(lang: "html")

```html
<template id="expand-panel">
  <div part="header"><slot name="trigger"></slot><div part="arrow">▼</div></div>
  <div part="content"><div part="inner"><slot name="content"></slot></div></div>
</template>
```

#set raw(lang: "rust")

投影的名字走 light DOM 的 `slot` 属性（`<div slot="trigger">`）。这层间接不是多余的：节点图靠它让节点*留在 light DOM 里*被 `<slot>` 投进 shadow root，元素不搬家，`ui-graph > ui-graph-node` 这类选择器才照常命中。细节见#link("/arch/docs/graph.html")[节点图]。

= 样式要写两份

shadow root 里的东西，页面样式表选不中 —— 样式隔离是特性不是缺陷。所以每个元素都有两份样式：

- `lib/css/<name>.css`：light DOM 侧。数据载体要藏（`pin`、`connection`）、光标、外边距
- JS 里内联的 `<style>`：shadow root 内部。盒子和图元长什么样

只有 `outline` 是例外：`nav.outline` 和它的 ✕ 按钮都在 light DOM 里（`outline.css` / `outline.js`），页面样式能直接够到。

shadow 挡的只是*选择器*，自定义属性照样继承。所以布局常量（`--header-h`）统一定义在 `theme.css` 里，shadow 内的 `header.css` 用 `var(--header-h)` 就能拿到 —— 顶栏在 shadow 里定高，两侧抽屉在 light DOM 里从它算偏移，两边对得上。

= 三笔代价

=== 解析顺序：元素比子节点先到

脚本在 `<head>`，自定义元素在解析到开标签的那一刻就 `constructor`，此刻它自己的子节点还没进 light DOM。直接 `querySelectorAll` 只会拿到空数组 —— 而且不报错，只是图上什么都没有。

所以两边都推迟到文档就绪再建：

```js
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => this.#build());
} else {
    this.#build();
}
```

=== 定义之前得能看

没跑 JS（或脚本 404）时，自定义元素就是个没样式的空标签。要么让它在 `:defined` 之前保持 `display: none`，要么留一句 fallback 文字让它解释自己 —— `graph.css` 两样都做了：

```css
&:not(:defined) > .graph-fallback { display: block; ... }
&:not(:defined) > ui-graph-node { display: none; }
```

`:defined` 是自定义元素独有的伪类，元素定义那一刻起规则自动失效。JS 建好图之后还会把 fallback 文字删掉。

=== 非 HTML 目标要能编

站点也会被导出成 pdf（Typst 的其它 target）。`html_elem` 在 `target() != "html"` 时一律返回纯内容、不产生标签；`custom_element` 的注册函数压根不会被收集（它在 `<head>` 里）。于是 `card` / `expand-panel` 这类函数都得自己准备一份非 HTML 的替身，`card_info` 落成 note-me 的 note、`expand-panel` 落成一行 `/ #trigger: #con`。

= 现有清单

下表是站点里全部自定义元素。写一个新的照着任一行抄：Typst 侧一个 `custom_element`，JS 侧一段 `define`，CSS 两份。

#table(
    columns: (auto, auto, auto, auto),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [来源], [*标签*], [*`<template>`*], [*JS / CSS*],
    [`lib/card.typ`], [`<ui-card>`], [`id="card"`], [`card.js` / `card.css`],
    [`lib/expand-panel.typ`], [`<expand-panel>`], [`id="expand-panel"`], [`expand-panel.js` / `expand-panel.css`],
    [`lib/code_block.typ`], [`<code-block>`], [`id="code-block"`], [`code_block.js` / `code_block.css`],
    [`lib/graph.typ`], [`<ui-graph>` + `<ui-graph-node>`], [共用一份注册], [`graph.js` / `graph.css`],
    [`lib/template.typ`], [`<toc-component>`], [无（fetch 整页）], [`toc.js` / `toc.css`],
    [`lib/template.typ`], [`<site-header>`], [无（fetch 整页）], [`header.js` / `header.css`],
)

两个细节别照抄错：

- *`code-block` 的 `<template>` 不走 `custom_element`*，而是随每个代码块落进正文（`show raw` 里生成的），JS 用 `getElementById` 取第一个。走 `custom_element` 更干净 —— 代价是要改一行。
- *`toc-component` 和 `site-header` 没有 `<template>`*：它们的 shadow root 全靠 `connectedCallback` 里 `fetch` 一份单独编译的页面（`toc.html` / `header.html`）再建。侧栏和顶栏各自就是一个页面（`lib/toc.typ`、`lib/header.typ`），所以拿得到。
  - 这两页 fetch 回来的是*整页* HTML，`<head>` 里还挂着全站那些 `<link>` 和 `<custom-element>` 的注册模板。直接丢给 `innerHTML` 的话那些 `<link>` 会被解析进 shadow root 并且*真的生效* —— 得先 `new DOMParser().parseFromString(html, 'text/html')` 再取 `.body.innerHTML`。
  - `<site-header>` 里的按钮因此在 shadow 里，别的脚本 `document.querySelector` 够不到。`header.js` 只负责「收到点击」，然后往 `document` 上发 `drawer-toggle` / `theme-toggle`；`theme.js` 换完主题再发回一个 `theme-applied`。和 `toc.js` 的 `drawer-close` 是同一个路子。

还有一个元素用了 `custom_element` 却*不是*自定义元素：`outline`（`lib/outline.typ`）。它注册 outline.css / outline.js，但产出的是普通的 `<nav>` 和 `<button>`，JS 走 `DOMContentLoaded` + `querySelectorAll` 直接操作 light DOM。`custom_element` 在这里只当「往 `<head>` 里塞脚本和样式」用 —— 所以名字有点名不副实，用的时候看的是那个注册函数，不是 `ele`。
