# 文档站深色模式 —— 设计

日期：2026-10-15

## 目标

给 `docs/typst` 生成的文档站加一套完整的深色主题：读者默认跟随系统 `prefers-color-scheme`，也可以用页面顶栏的按钮手动切换，选择记在 `localStorage`。覆盖范围是全站 —— 正文、两侧侧栏、开关按钮、代码块、卡片、折叠面板、节点图，不留半深半浅的页面。

## 现状

- 站点是 Typst 导出的静态 HTML bundle。所有样式是 `docs/typst/lib/css/` 下的普通 CSS，按组件一个文件，由 `lib/contents.typ:40` 的 `import_style` 在每页 `<head>` 里 link 进来。
- **没有颜色变量层**。颜色全是散落的十六进制字面量，出现在 `style.css`、`template.css`、`outline.css`、`toc.css`、`card.css`、`code_block.css`、`graph.css`、`expand-panel.css`，以及 `lib/js/graph.js:90-117`、`:473-484` 的内联 `<style>` 字符串里。
- 四个组件渲染在 shadow root 里：`toc-component`、`code-block`、`expand-panel`、`ui-graph`。页面 CSS 进不去。
- 高亮器是死代码（`lib/code_block.typ:29-30` 的 hljs `<link>`/`<script>` 被注释掉了，`lib/js/code_block.js` 末尾的高亮逻辑同样被注释）。代码块是纯等宽文字，没有语法主题要换。
- `dev/docs/layout.typ` 记了三个结构性坑：块级内容被包进 `.block`、全局 `line-height: 0.8`、`.block { width: fit-content }`。改样式要绕开它们。

## 方案

### 机制

一层语义化 CSS 变量声明在 `:root` 上，深色值只在 `:root[data-theme="dark"]` 里出现一次。head 里的阻塞脚本把生效主题解析成 `data-theme` 写在 `document.documentElement` 上。

被否掉的两个：

- **每个 CSS 文件追加 `[data-theme="dark"]` 覆盖块**。改动最小，但每个颜色从此在两个文件里各写一遍，改一处忘一处必然漂移。
- **编译出两份样式表、切 `<link>` 的 href**。运行时无重复，但要双份文件、构建系统要改，而且够不到 `ui-graph` shadow root 里的内联 `<style>`（那需要 JS 字符串手术）。

选中的方案有一个代价：**深色模式依赖 JS**，因为主题由脚本而不是 media query 解析。这在本项目可接受 —— 侧栏、目录抽屉、节点图、复制按钮本来就全是 JS 驱动的，没 JS 的读者今天也拿不到这个站。换来的是 `color-scheme` 正确生效（滚动条、UA 控件）以及零闪白。

### token 层

新文件 `docs/typst/lib/css/theme.css`，由 `lib/template.typ:41` 在 `style.css` 之前 link。只两个块：

```css
:root { color-scheme: light;  /* 浅色值 */ }
:root[data-theme="dark"] { color-scheme: dark; /* 深色值 */ }
```

token 从代码里实际存在的字面量推导，不新造：

| token | 浅色 | 用在哪 |
|---|---|---|
| `--bg` | `#fdfdfd` | `body`、`toc-component`、`.outline`、`code-block` |
| `--surface` | `#fff` | 开关按钮、`.outline-close`、`.drawer-close` |
| `--fg` | `#19181f` | 正文、图节点文字、`li > button.open::after` |
| `--muted` | `#565565` | `.outline`、图 fallback 文字 |
| `--subtle` | `#7a7c86` | 图引脚标签（`graph.js:114`） |
| `--accent` | `#11566c` | 当前条目、图边框与连线 |
| `--link` | `#3964fe` | `a` |
| `--border` | `#e2e3e8` | 开关按钮、`ui-graph`、`.outline-close` |
| `--border-soft` | `#d7d9e2` | `ui-graph` 边框、图节点盒 |
| `--hover` / `--active` | `#eff0f3` / `#e4e5ea` | 按钮 hover、`li.open > *:first-child` |
| `--panel` / `--panel-head` | `#efefef` / `gainsboro` | `expand-panel` |
| `--code-bg` / `--code-inline-bg` | `#fdfdfd` / `#eff3f8` | 代码块、行内 `code` |
| `--shadow-1` / `--shadow-2` | `#59556533` / `rgba(0,0,0,.12)` | 代码块、卡片 hover |
| `--overlay` | `rgba(0,0,0,.5)` | `.drawer-overlay` |
| `--highlight-bg` / `--highlight-fg` | `rgba(255,152,0,.08)` / `#ff9800` | 锚点 hover 高亮 |
| `--card-info-*`、`--card-tip-*`、`--card-attention-*` | 现有的 `#D1ECF1`/`#155724` 等三色组 | `ui-card[type=…]` |
| `--graph-canvas`、`--graph-box`、`--graph-ctrl`、`--graph-reset` | `#fbfbfd`、`#fff`、`#11566c` | `graph.css` + `graph.js` shadow |
| `--header-h` | `48px` | 顶栏、抽屉偏移量 |

深色值是按对比度重新挑的，不是把浅色值调暗：`--accent` 换成更亮的青，`--link` 换成更亮的蓝紫，`--fg` 用近白，卡片改成低彩度着色底 + 浅色文字。卡片 `::part(icon)` 保留 `#17A2B8`、`#28A745`、`#FFC107` —— 这三个在深色上都还读得清。

接着把 `outline.css`、`toc.css`、`card.css`、`code_block.css`、`graph.css`、`expand-panel.css`、`style.css`（`a` 的颜色）、`template.css`（`body` 的颜色）里的字面量换成 `var(--x)`。

### 主题解析

新文件 `lib/js/theme.js`，由 `lib/template.typ:40` 和 `style.js` 一起用现成的 `import_script` 引入。head 里的同步外链脚本会阻塞渲染，跑在正文之前，`data-theme` 在第一帧就是对的，不会闪一下白底。

```js
// 尽早定下主题:这是 head 里的阻塞脚本,跑在正文渲染之前,
// 所以 data-theme 在第一帧就是对的,不会闪一下白。
const KEY = 'r2g-theme';
const root = document.documentElement;

function system() {
    return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}
function stored() {
    try { return localStorage.getItem(KEY); } catch { return null; }
}

function apply(theme) { root.dataset.theme = theme; }

apply(stored() === 'dark' || stored() === 'light' ? stored() : system());

// 没有手动选择过时,跟随系统实时变化
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
    if (!stored()) apply(e.matches ? 'dark' : 'light');
});

document.addEventListener('DOMContentLoaded', () => {
    document.querySelector('.theme-toggle')?.addEventListener('click', () => {
        const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
        try { localStorage.setItem(KEY, next); } catch {}
        apply(next);
    });
});
```

三个细节：

- head 脚本运行时按钮还不存在（Typst 把它写进 body），所以接线推迟到 `DOMContentLoaded`，而主题属性在顶层就设好。两半的生命周期本来就不同。
- `localStorage` 访问包在 `try/catch` 里 —— 某些 `file://` 和隐私配置下它会抛错，这里抛错会让页面停在错的主题上，而不只是丢持久化。
- `matchMedia` 的监听只在**没有**手动选择时重新应用。没有这道闸，手动切到浅色之后再改系统设置会把读者的选择悄悄冲掉。

按钮的 `aria-label` 由 `theme.js` 改成目标状态（「切换到深色模式」/「切换到浅色模式」），不是固定的「切换主题」。

### 顶栏

`lib/outline.typ` 本来就输出两个 ☰ 按钮，顶栏在那里包一层，而不是新加一个输出点：

```html
<header class="site-header">
  <div class="header-left">
    <button class="toc-toggle">☰</button>
    <a class="site-title" href="<base>index.html">rust2genshin</a>
  </div>
  <div class="header-right">
    <button class="theme-toggle" aria-label="切换主题">…</button>
    <button class="outline-toggle">☰</button>
  </div>
</header>
```

顶栏是 `body` 的直接子元素（`template.typ` 里 `outline.outline()` 在 `main` 之外），不落在 Typst 插的 `.block`/`.container` 包装里，grid 行位置可预测。

`outline.css` 是断点最敏感的文件，三处连带影响必须先说清楚：

**1. `body` 是 grid，多一个子元素不是简单堆叠。** `body { display: grid }` 加各断点的 `grid-template-columns`。裸 `<header>` 会落进 row 1 的第一列，把 `main` 挤偏。解法：`body { grid-template-rows: auto 1fr }` 加 `header { grid-column: 1 / -1 }`，顶栏横跨三列，其余子元素原样落进 row 2。

**2. 顶栏必须 `position: sticky; top: 0`。** 两个 ☰ 现在是 `position: fixed`，滚动时始终够得着。挪进普通顶栏会让它们在任何长于视口的页面上滚没，是回退。sticky 保住了行为，但也意味着顶栏现在会盖住内容，于是五个偏移量要动：

| 规则 | 现在 | 改成 |
|---|---|---|
| `.outline`（≥768px） | `top: 32px; height: calc(100vh - 64px)` | `top: calc(var(--header-h) + 16px); height: calc(100vh - var(--header-h) - 32px)` |
| `toc-component`（窄屏抽屉） | `top: 0; max-height: 100vh` | `top: var(--header-h); max-height: calc(100vh - var(--header-h))` |
| `.outline`（窄屏抽屉） | 同上 | 同上 |
| `.drawer-overlay` | `top: 0` | `top: var(--header-h)` |
| `.toc-toggle` / `.outline-toggle` | `position: fixed; top: 16px; left/right: 16px` | `position: static`（进顶栏的常规流） |

全部走同一个 `--header-h`，以后改顶栏高度是一行的事。

**3. `main { padding-top: 48px }` 删掉。** 它存在的唯一原因是给浮动 ☰ 让位。顶栏独占一个 grid 行之后不再重叠，留着只会在每个窄屏页面上多 48px 死白。

主题按钮的图标用内联 SVG 的 `mask` + `currentColor`，配色自动跟着走。

### shadow root 与 SVG

**shadow root 不需要新管线。** 自定义属性会跨 shadow 边界继承，`:root` 上的 `--bg` 一样进得去：

- `toc.css` 已经是页面 CSS；`toc.js:13-15` 把取到的文件 adopt 进 shadow root，变量照样流进去。
- `code-block`、`expand-panel`、`card` 由页面的 `::part()` 样式，只改值。
- `ui-graph` 的渲染样式是 `graph.js:90-117`、`:473-484` 里的内联 `<style>` 字符串（节点盒 `#fff`/`#c9cbd4`、文字 `#19181f`、连线和控件 `#11566c`、引脚标签 `#7a7c86`、reset 按钮 `rgba(255,255,255,.9)`）。全换成 `var(--graph-*)`。`.g-edge` 是 SVG，但 shadow tree 里的 SVG 仍然从宿主解析自定义属性，不需要 JS。

**`var()` 在 `url("data:image/svg+xml,…")` 里不生效。** 两个图标的 URI 里烤死了十六进制：

- `outline.css` 的 `.toc-toggle` / `.outline-toggle` —— `stroke='%2319181f'`。解法：把图标挪到 `::before` 伪元素，用 `stroke='currentColor'` 的 `mask-image`，由按钮的 `color` 承载 token。`☰` 字形本来就用 `font-size: 0` 藏了，伪元素正好占那个位置，不用改标记。hover/active 底色仍归按钮的 `background-color`，不再和伪元素抢。
- `toc.css` 的 `li > button.open::after` —— 本来就是 `stroke='currentColor'`，补一句 `color: var(--fg)` 即可。

`card.css` 的 mask 本来就是 `fill='currentColor'` + `background-color: currentColor`，只有 `::part(icon)` 的颜色需要 token，URI 不用动。

**一处故意不改：** `code-block[lang="output"] { background-color: black !important; color: white }` 是模拟终端，浅色模式下它本来就是深底，两个主题下都保持黑。

## 验收

这个项目没有 CSS 测试手段，验收是一份清单，对着 `cargo run -p rust2genshin-docs --bin watch` 跑：

- 两个主题 × 三个断点（375 / 800 / 1280px）：侧栏、抽屉加遮罩、页内目录、顶栏、代码块、卡片、折叠面板，以及一个 `ui-graph`（拖拽、缩放、reset —— reset 按钮的 hover 是手写色的）。
- 切换后在页面跳转和重载后保持；没有手动选择时改系统设置会实时跟随；有手动选择时改系统设置不动。
- 深色系统下首次打开不闪浅色。
- 对比度：正文和 `.outline` 在深色下 ≥4.5:1，`--accent`/`--link`/边框 ≥3:1。
- 回归过一遍 `dev/docs/layout.typ` 的三个坑 —— 新顶栏这一行最可能撞上 `.block` 包装。

## 文档

在 `dev/docs/layout.typ` 补第四节「主题变量」。那一页记的就是「结构性原因导致 CSS 在这里反复出问题」，而「自定义属性跨 shadow 边界、普通选择器不跨」正好属于这一类。