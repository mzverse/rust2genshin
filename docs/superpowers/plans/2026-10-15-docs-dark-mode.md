# 文档站深色模式 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 `docs/typst` 生成的文档站加一套跟随系统、可手动切换并持久化的深色主题，覆盖全站组件。

**Architecture:** 在 `:root` 上声明一层语义化 CSS 变量，浅色值一份、深色值一份（`:root[data-theme="dark"]`）。head 里的阻塞脚本把生效主题解析成 `document.documentElement` 上的 `data-theme`，跑在第一帧之前。四个 shadow DOM 组件（`toc-component`、`code-block`、`expand-panel`、`ui-graph`）不需要额外管线 —— 自定义属性跨 shadow 边界继承，这是普通选择器做不到而变量能做到的唯一一件事。

**Tech Stack:** Typst 0.15.1（`--features bundle,html --format bundle`）、原生 CSS 自定义属性、shadow DOM、Vanilla JS。

---

## 本项目的测试形态（先读）

这个项目**没有 CSS 测试框架**，CSS 改动不报错、不失败，只在浏览器里悄悄不生效（`dev/docs/layout.typ` 记的就是这件事）。所以本计划里：

- 「写失败测试 → 跑 → 实现 → 跑」的机械形式**不存在**，用下面两种可检查的断言替代：
  - **文本断言**：`grep` 某段字面量应当消失 / 某行应当存在。失败即非零退出码。
  - **构建断言**：`typst compile` 退出码为 0（`main.typ` 生成阶段的 Typst 语法错误在这里暴露）。
- **视觉断言**集中在 Task 9 的验收清单，只能人工在浏览器里过。

其余流程不变：每步一个动作（2–5 分钟）、完整代码、每 task 一次提交。

## 构建与验证命令

**本机 `cargo run` 不可用** —— 依赖的 build script（`libc`、`windows_x86_64_gnu`）以 `STATUS_ACCESS_VIOLATION` 崩溃，与本任务无关。所以全计划直接调 `typst`，等价于 `docs/src/typst.rs:47-58` 做的事：

```shell
cd F:/rust2genshin
export PATH="$PATH:/c/Users/53548/AppData/Local/Microsoft/WinGet/Links"
typst compile --root docs/typst ./docs/typst/main.typ target/debug/docs --features bundle,html --format bundle
```

`docs/typst/main.typ` 由 `docs/src/typst.rs:21 build()` 生成：它遍历整个 `docs/typst` 树，为每个文件写一行 `#bundle_document("/...")` 或 `#bundle_asset("/...")`。**Typst 的 bundle 导出只发清单里有的东西。** 所以：

- 新增任何 **asset**（`.css` / `.js`，即 `.typ` 以外的文件）必须同时在 `main.typ` 加一行 `#bundle_asset`，否则引用它的 `<link>` / `<script src>` 静默 404 —— 构建照样退出 0，样式只是不生效。这是 Task 1 实际踩到的坑。
- 新增或删除 `.typ` 页面同理，需要 `#bundle_document` 行。
- `main.typ` 是 gitignored 的（`docs/typst/.gitignore:1`）。加在生成器会给出的排序位置上，**不要提交**；下次 `cargo run` 重新生成时结果一致。

本计划只新增一个 asset：`lib/js/theme.js`（Task 2）。`lib/css/theme.css`（Task 1）已加好。

构建成功时退出码 0，输出里只有 `no text within stars` 这类已知告警。

人工看效果：`cargo run -p rust2genshin-docs --bin watch`（本机同样受 cargo 问题影响，改用 `typst watch --root docs/typst ./docs/typst/main.path target/debug/docs --features bundle,html --format bundle` 加浏览器手动刷新）。

---

## 文件结构

| 文件 | 动作 | 职责 |
|---|---|---|
| `docs/typst/lib/css/theme.css` | **新建** | 全部 token 的唯一定义处；浅色/深色各一块，另放跨 shadow 用的布局常量 |
| `docs/typst/lib/js/theme.js` | **新建** | 在第一帧前解析并设置 `data-theme`，接线切换按钮 |
| `docs/typst/lib/template.typ` | 改 | link `theme.css`、script `theme.js` |
| `docs/typst/lib/outline.typ` | 改 | 输出 `<header class="site-header">` |
| `docs/typst/lib/css/style.css` | 改 | 字面量 → `var()` |
| `docs/typst/lib/css/template.css` | 改 | 字面量 → `var()` |
| `docs/typst/lib/css/outline.css` | 改 | 字面量 → `var()`；顶栏与抽屉偏移；按钮图标改 mask |
| `docs/typst/lib/css/toc.css` | 改 | 字面量 → `var()` |
| `docs/typst/lib/css/card.css` | 改 | 三套卡片配色 → `var()` |
| `docs/typst/lib/css/code_block.css` | 改 | 字面量 → `var()` |
| `docs/typst/lib/css/expand-panel.css` | 改 | 字面量 → `var()` |
| `docs/typst/lib/css/graph.css` | 改 | 字面量 → `var()` |
| `docs/typst/lib/js/graph.js` | 改 | shadow 内联 `<style>` 字面量 → `var()` |
| `docs/typst/dev/docs/layout.typ` | 改 | 加第四节「主题变量」 |

**为什么单独一个 `theme.css` 而不是散进各组件文件**：每个颜色只在两个相邻的块里各写一次，「X 是什么颜色」永远是一次 grep。深色值只存在于 `:root[data-theme="dark"]` 一处，不存在双份覆盖块漂移的问题。

---

### Task 1: 建立 token 层 ✅ 已完成（commit `44bc4a`、`37344cc`、`7b0f768`）

> **实施结果与本计划初稿有三处出入，后续 task 以 `docs/typst/lib/css/theme.css` 的实际内容为准：**
>
> 1. **新增 asset 必须登记进 `docs/typst/main.typ`。** 本计划初稿以为「本计划不新增 `.typ` 文件，所以 `main.typ` 不用重新生成」——错的。`main.typ` 是 `docs/src/typst.rs:21 build()` 生成的**清单**，站点里每个文件都要一行 `#bundle_document` / `#bundle_asset`，Typst 的 bundle 导出只发清单里有的东西。新增任何 **asset**（`.css`/`.js`）都必须加一行，否则 `<link>`/`<script src>` 静默 404。该文件是 gitignored 的，加在生成器会给出的位置上，不要提交。
> 2. **token 名按角色命名，不按下标。** `--shadow-1`/`--shadow-2` → `--shadow-rest`/`--shadow-hover`，另加 `--shadow-drawer`（抽屉投影 `.18`）与 `--shadow-float`（顶栏按钮与卡片 hover 的 `.10`，两处同值共用一个）。
> 3. **补齐了初稿漏掉的 token。** `code_block.css` 的 10 个字面量（语言徽标、复制按钮三种状态）、两处共用的关闭按钮 `#f0f0f0`/`#ddd`、`graph.css` 的 fallback 底色都补上了；`--graph-pin` 因为与 `--subtle` 同值而删掉；`--graph-edge` → `--graph-accent`。

**Files:**
- Create: `docs/typst/lib/css/theme.css`
- Modify: `docs/typst/lib/template.typ:41`

- [x] **Step 1: 写 `theme.css`**

- [x] **Step 2: 在 `template.typ` 里 link 它（必须在 `style.css` 之前）**

`docs/typst/lib/template.typ:40-42` 现在是：

```typst
            import_script("/lib/js/style.js");
            import_style("/lib/css/style.css");
            import_style("/lib/css/template.css");
```

改成：

```typst
            import_script("/lib/js/theme.js");
            import_script("/lib/js/style.js");
            import_style("/lib/css/theme.css");
            import_style("/lib/css/style.css");
            import_style("/lib/css/template.css");
```

- [x] **Step 3: 跑构建**

```shell
cd F:/rust2genshin && export PATH="$PATH:/c/Users/53548/AppData/Local/Microsoft/WinGet/Links" && typst compile --root docs/typst ./docs/typst/main.typ target/debug/docs --features bundle,html --format bundle; echo "EXIT=$?"
```

预期：`EXIT=0`，输出只有已知的 `no text within stars` 告警。

- [x] **Step 4: 断言 token 文件被复制进产物并被每页 link**

```shell
cd F:/rust2genshin && ls target/debug/docs/lib/css/theme.css && grep -c 'href="/lib/css/theme.css"' target/debug/docs/user/index.html target/debug/docs/index.html
```

预期：两个文件都存在；`grep -c` 两行各输出 `1`。

- [x] **Step 5: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/lib/css/theme.css docs/typst/lib/template.typ && git commit -m "feat(docs): add color token layer for theming"
```

---

### Task 2: 主题解析脚本

**Files:**
- Create: `docs/typst/lib/js/theme.js`
- Modify: `docs/typst/lib/template.typ:40`（Task 1 已加 `import_script`，本 task 只写文件本体）

- [ ] **Step 1: 写 `theme.js`**

```js
// 在第一帧之前定下主题。这是 <head> 里的阻塞脚本,跑在正文渲染之前,
// 所以 data-theme 从一开始就是对的 —— 不用 inline script,也不用
// document.write,localStorage 抛错也不会把页面卡在错的主题上。
//
// 深色模式因此依赖 JS。侧栏、目录抽屉、节点图、复制按钮本来就全是 JS
// 驱动的,没 JS 的读者今天也拿不到这个站。
const KEY = 'r2g-theme';
const root = document.documentElement;

function system() {
    return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

// 某些 file:// 和隐私配置下 localStorage 直接抛。那就让主题不持久,
// 而不是让整个脚本挂掉。
function stored() {
    try {
        return localStorage.getItem(KEY);
    } catch {
        return null;
    }
}

function apply(theme) {
    root.dataset.theme = theme;
}

const saved = stored();
apply(saved === 'dark' || saved === 'light' ? saved : system());

// 跟着系统实时变 —— 但只在读者没有手动选过的时候。没有这道闸,手动切到
// 浅色之后再改系统设置,会把读者的选择悄悄冲掉。
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
    if (!stored()) apply(e.matches ? 'dark' : 'light');
});

// 按钮是 Typst 写进 body 的,脚本在 head 里跑的时候它还不存在。
const LABEL = { dark: '切换到浅色模式', light: '切换到深色模式' };

document.addEventListener('DOMContentLoaded', () => {
    const button = document.querySelector('.theme-toggle');
    if (!button) return;
    // aria-label 说的是「按下去会变成什么」,不是「现在是什么」。
    button.setAttribute('aria-label', LABEL[root.dataset.theme]);
    button.addEventListener('click', () => {
        const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
        try {
            localStorage.setItem(KEY, next);
        } catch {
            /* 不持久,下次访问回到系统设置 */
        }
        apply(next);
        button.setAttribute('aria-label', LABEL[next]);
    });
});
```

- [ ] **Step 2: 断言脚本被复制进产物并被每页引用**

```shell
cd F:/rust2genshin && ls target/debug/docs/lib/js/theme.js && grep -c 'src="/lib/js/theme.js"' target/debug/docs/user/index.html
```

预期：文件存在；`grep -c` 输出 `1`。

- [ ] **Step 3: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/lib/js/theme.js && git commit -m "feat(docs): resolve theme before first paint"
```

---

### Task 3: 组件样式改用 token（纯重构，外观必须不变）

这个 task 不引入深色，只把字面量换成 `var()`。**改完页面必须和改之前一模一样** —— 这是后面所有 task 的地基。

例外清单（保持字面量）：

- `code_block.css` 里 `code-block[lang="output"]` / `code.language-output` 的 `background-color: black` + `color: white`：模拟终端，本来就是深底，两个主题都保持黑。
- `card.css` 里三段 `mask-image` 的 data URI：里面是 `fill='currentColor'`，没有烤死的颜色，本来就跟着 `color` 走。
- `toc.css` 里 `li > button.open::after` 的 data URI：已经是 `stroke='currentColor'`，Task 6 只补 `color`，URI 不动。
- `style.css` 里 `p::before { content: "DO NOT USE <p>" ; color: red }`：调试哨兵，不参与配色。

- [ ] **Step 1: 改 `style.css`**

`a { color: #3964fe; }`（`docs/typst/lib/css/style.css:33`）改成：

```css
a {
    color: var(--link);
    text-decoration-line: none;
    &:hover {
        text-decoration-line: underline;
    }
}
```

`.highlight`（`style.css:5`）改成：

```css
.highlight {
    outline: 2px solid var(--highlight-fg);
    outline-offset: 4px;
    background: var(--highlight-bg);
    transition: background 0.25s, outline-color 0.25s;
    border-radius: 4px;
}
```

- [ ] **Step 2: 改 `template.css`**

整个文件改成：

```css
main {
    flex-direction: column;
    min-height: 100vh;
    margin: 12pt;
}

body {
    font-family: quote-cjk-patch, Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
    color: var(--fg);
    font-size: 12pt;
}
```

- [ ] **Step 3: 改 `expand-panel.css`**

`background: #efefef`（:6）、`border: 1pt solid black`（:4）、`background: gainsboro`（:11）、`background: silver`（:16）全部换 token，文件改成：

```css
expand-panel {
    display: block;
    border: 1pt solid var(--border);
    margin: 6pt 0;
    border-radius: 6pt;
    background: var(--panel);

    &::part(header) {
        border-radius: 6pt;
        cursor: pointer;
        background: var(--panel-head);
        padding: 9pt 9pt;
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    &::part(header):hover {
        background: var(--active);
    }

    &::part(arrow) {
        transition: transform 0.2s;
        font-size: 12px;
        transform: rotate(90deg);
        user-select: none;
    }

    &::part(content) {
        border-radius: 6pt;
        overflow: hidden;
        transition: max-height 0.25s;
        max-height: 0;
    }

    & > [slot="content"] {
        padding: 0 9pt;
        margin: 6pt 0;
    }

    &[open] {
        &::part(arrow) {
            transform: rotate(0deg);
        }

        &::part(content) {
            max-height: unset;
        }
    }
}
```

注意 `silver`（hover）换成 `--active` 而不是新加 token —— 浅色下 `#e4e5ea` 与 `silver` 同属中性灰这一档，视觉等价。

- [ ] **Step 4: 改 `graph.css`**

`border: 1px solid #d7d9e2; background-color: #fbfbfd;`（:19-20）换 token；fallback 那块的 `border: 1px dashed #c9cbd4; background-color: #f7f8fa; color: #565565;`（:41-43）换成 `var(--graph-border)` / `var(--graph-fallback-bg)` / `var(--muted)`。

- [ ] **Step 5: 改 `code_block.css`**

`background-color: #fdfdfd`（:8）→ `var(--code-bg)`；`box-shadow: 0 4px 12px #59556533`（:10）→ `var(--shadow-rest)`；`box-shadow: 0 8px 30px rgba(0, 0, 0, 0.12)`（:14）→ `var(--shadow-hover)`；`background-color: #eff3f8 !important`（:100）→ `var(--code-inline-bg) !important`。

代码块控件（Task 1 补齐的 8 个 token，本步消费）：

| 位置 | 字面量 | token |
|---|---|---|
| :66-67 语言徽标 | `#2563eb` / `#ffffff` | `--badge-bg` / `--badge-fg` |
| :75-76 复制按钮 | `#7c3aed` / `#ffffff` | `--copy-bg` / `--copy-fg` |
| :88-89 复制按钮 hover | `#6d28d9` / `#ede9fe` | `--copy-hover-bg` / `--copy-hover-fg` |
| :92-93 已复制状态 | `#98c379` / `#ffffff` | `--copied-bg` / `--copied-fg` |

`[lang="output"]` 与 `pre:has(code.language-output)` 的 `black`/`white`（:127-131）**不动** —— 那是模拟终端，本来就是深底，两个主题下都保持黑。

- [ ] **Step 6: 改 `card.css` 的三套配色**

`ui-card` 里三段 `&[type="…"]` 改成：

```css
    &[type="info"] {
        background-color: var(--card-info-bg);
        border: 1px solid var(--card-info-border);
        color: var(--card-info-fg);
    }
    &[type="info"]::part(icon) {
        color: var(--card-info-icon);
    }
```

`tip` 用 `--card-tip-*`，`attention` 用 `--card-attention-*`，同理。三段 `mask-image` 的 data URI 原样保留（里面是 `currentColor`）。`ui-card:hover` 的 `box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1)`（:16）→ `var(--shadow-float)`。

- [ ] **Step 7: 改 `toc.css`**

`#f0f0f0`（:16）、`#ddd`（:17）→ `var(--close-bg)` / `var(--close-border)`（与 `.outline-close` 共用同一对 token，浅色下这两处本来就在抄同一个色值）；`#eff0f3`（:99、:112、:122、:126）→ `var(--hover)`；`#e4e5ea`（:130）→ `var(--active)`；`#19181f`（:119）→ `var(--fg)`；`#11566c`（:147、:156）→ `var(--accent)`。

- [ ] **Step 8: 改 `outline.css`**

除 Task 6 会重写的按钮和顶栏部分外，本步先做：

- :35 `body { background-color: #fdfdfd }` → `var(--bg)`
- :50 `.drawer-overlay { background-color: rgba(0, 0, 0, 0.5) }` → `var(--overlay)`
- :16 `toc-component { background-color: #fdfdfd }` → `var(--bg)`
- :141 `.outline-close { background-color: #f0f0f0; border: 1px solid #ddd }` → `var(--close-bg)` / `var(--close-border)`
- :104 `.outline { color: #565565 }` → `var(--muted)`；`:133` 的 `background-color: #fdfdfd` → `var(--bg)`
- :95-96 `.toc-toggle:hover/.outline-toggle:hover/.…:active { background-color: #eff0f3 / #e4e5ea }` → `var(--hover)` / `var(--active)`
- :24 和 :127 两处 `box-shadow: … rgba(0, 0, 0, .18)`（`toc-component` 与 `.outline` 的抽屉投影）→ `var(--shadow-drawer)`
- :83 `.toc-toggle/.outline-toggle` 的 `box-shadow: 0 1px 4px rgba(0, 0, 0, .10)` → `var(--shadow-float)`（Task 5 会把这两个按钮挪进顶栏，这条规则保留）
- :251 `.outline li.current > :first-child * { color: #11566c }` → `var(--accent)`

- [ ] **Step 9: 断言：除例外清单外没有裸颜色字面量残留**

```shell
cd F:/rust2genshin/docs/typst/lib/css && grep -nE '#[0-9a-fA-F]{3,8}\b' style.css template.css toc.css outline.css card.css code_block.css graph.css expand-panel.css | grep -v 'theme.css'
```

预期：输出里每一条都能对上例外清单（`p::before` 的 `red` 不匹配这个正则；`code_block.css` 的 `black`/`white` 是关键字不是 hex，所以应该**一条都不剩**）。

如果还有输出，逐条判断：是 data URI 里烤死的颜色（记下来，Task 5 处理），还是漏改的 token 引用。

- [ ] **Step 10: 构建 + 视觉比对**

```shell
cd F:/rust2genshin && export PATH="$PATH:/c/Users/53548/AppData/Local/Microsoft/WinGet/Links" && typst compile --root docs/typst ./docs/typst/main.typ target/debug/docs --features bundle,html --format bundle; echo "EXIT=$?"
```

浏览器打开 `target/debug/docs/user/index.html`：页面应与改动前**完全一致**。特别看 —— 代码块底色、行内 `code` 底色、折叠面板、info/tip/attention 卡片、节点图的画布和节点盒子。

- [ ] **Step 11: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/lib/css && git commit -m "refactor(docs): reference color tokens instead of literals"
```

---

### Task 4: `ui-graph` 的 shadow 样式改用 token

**Files:**
- Modify: `docs/typst/lib/js/graph.js:86-118`（`NODE_CSS`）、`docs/typst/lib/js/graph.js:473-484`

节点图整个渲染器在 `ui-graph` 的 shadow root 里，页面 CSS 选不中。但自定义属性会从宿主 `ui-graph` 继承进来 —— 所以把内联 `<style>` 里的字面量换成 `var(--graph-*)` 就够了，不需要 JS 传值。SVG 里的 `stroke` 同理。

- [ ] **Step 1: 改 `NODE_CSS`**

`docs/typst/lib/js/graph.js:86-118` 的模板字符串里替换四处：

- `background: #fff` → `background: var(--graph-box)`
- `border: 1.5px solid #c9cbd4` → `border: 1.5px solid var(--graph-border)`
- `line-height: 21px; color: #19181f` → `line-height: 21px; color: var(--fg)`
- `color: #7a7c86`（`.g-label`）→ `color: var(--subtle)`（`--graph-pin` 已删，同值不同名会漂）
- `.g-ctrl { fill: #11566c }` → `fill: var(--graph-accent)`
- `.g-data { fill: #fff; stroke: #11566c }` → `fill: var(--graph-box); stroke: var(--graph-accent)`
- `:host(:hover) .g-box { border-color: #11566c }` 与 `:host(.hot) .g-box { border-color: #11566c; ... }` → `var(--graph-accent)`

- [ ] **Step 2: 改 shadow 里的连线与 reset 按钮样式**

`docs/typst/lib/js/graph.js:473-484` 替换三处：

- `.g-edge { fill: none; stroke: #11566c; ... }` → `stroke: var(--graph-accent)`
- `.g-reset { ... border: 1px solid #c9cbd4; background: rgba(255, 255, 255, .9); color: #11566c; ... }` → `border: 1px solid var(--graph-border); background: var(--graph-reset-bg); color: var(--graph-accent);`
- `.g-reset:hover { background: #fff; }` → `background: var(--graph-box);`

- [ ] **Step 3: 断言没有漏网的字面量**

```shell
cd F:/rust2genshin && grep -nE '#[0-9a-fA-F]{3,8}\b|rgba?\(255, ?255, ?255' docs/typst/lib/js/graph.js
```

预期：无输出。

- [ ] **Step 4: 构建 + 视觉比对**

跑 Task 3 Step 10 的构建命令，然后打开含节点图的页面（`docs/typst/arch/low/dataflow.typ` 等，产物 `target/debug/docs/arch/low/dataflow.html`）。拖一个节点、缩放、按 reset —— 外观应与改动前一致。

- [ ] **Step 5: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/lib/js/graph.js && git commit -m "refactor(docs): theme node graph through CSS tokens"
```

---

### Task 5: 顶栏标记与布局偏移

**Files:**
- Modify: `docs/typst/lib/outline.typ:22-24`
- Modify: `docs/typst/lib/css/outline.css:1`（`body` 的 grid）、`:14`（`toc-component`）、`:50`（`.drawer-overlay`）、`:70-96`（两个开关按钮）、`:100`（`main`）、`:104-139`（`.outline`）、`:158+` 与 `:212+`（两个 media query）

这一 task 动的是 `outline.css` 里断点最敏感的部分。三个必然后果：

1. `body` 是 grid，多一个子元素不是简单堆叠 —— 需要 `grid-template-rows: auto 1fr` 加 `header { grid-column: 1 / -1 }`，顶栏横跨三列，其余子元素原样落进 row 2。
2. 顶栏必须 `position: sticky; top: 0` —— 两个 ☰ 现在是 `position: fixed`，滚动时始终够得着；挪进普通顶栏会让它们在任何长于视口的页面上滚没，是回退。sticky 保住了行为，但也意味着顶栏现在盖住内容，于是五个偏移量要跟着 `--header-h` 走。
3. `main { padding-top: 48px }` 删掉 —— 它存在的唯一原因是给浮动 ☰ 让位；顶栏独占一个 grid 行之后不再重叠，留着只会在每个窄屏页面多 48px 死白。

- [ ] **Step 1: 在 `outline.typ` 里输出顶栏**

`docs/typst/lib/outline.typ:22-24` 现在是：

```typst
    html_elem("button", attrs: (class: "toc-toggle"))[☰]
    html_elem("button", attrs: (class: "outline-toggle"))[☰]
```

改成：

```typst
    // 顶栏。它是 body 的直接子元素(template.typ 在 main 之外调 outline()),
    // 所以不落在 Typst 插的 .block/.container 包装里,grid 行位置可预测。
    // sticky —— ☰ 原来是 position: fixed,滚动时始终够得着;挪进普通顶栏
    // 会让它们滚没。
    html_elem("header", attrs: (class: "site-header"), {
        html_elem("div", attrs: (class: "header-left"), {
            html_elem("button", attrs: (class: "toc-toggle"))[☰]
            html_elem("a", attrs: (class: "site-title", href: base_path + "/index.html"))[rust2genshin]
        })
        html_elem("div", attrs: (class: "header-right"), {
            html_elem("button", attrs: (class: "theme-toggle"), attrs: (aria-label: "切换主题"))[]
            html_elem("button", attrs: (class: "outline-toggle"))[☰]
        })
    })
```

`theme.js` 用 `document.querySelector('.theme-toggle')` 找按钮，`aria-label` 在 `DOMContentLoaded` 时被改写成目标状态。

- [ ] **Step 2: `body` 加一行 grid，顶栏横跨**

`docs/typst/lib/css/outline.css:35` 的 `body` 块，在 `grid-template-columns` 紧后面加 `grid-template-rows: auto 1fr;`。

- [ ] **Step 3: 写顶栏样式**

在 `outline.css` 的 `.toc-toggle, .outline-toggle` 规则**之前**插入：

```css
/* 顶栏。sticky,高度由 --header-h 定死 —— 下面所有抽屉/侧栏的偏移
 * 都从这一个值算,以后改高度只改一处。 */
.site-header {
    position: sticky;
    top: 0;
    z-index: 1000;
    grid-column: 1 / -1;
    height: var(--header-h);
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 16px;
    background-color: var(--bg);
    border-bottom: 1px solid var(--border);
}
.header-left, .header-right {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
}
.site-title {
    color: var(--fg);
    font-weight: 700;
    text-decoration-line: none;
    white-space: nowrap;
}
.site-title:hover {
    text-decoration-line: underline;
}
/* 主题按钮。图标走 mask + currentColor,配色自动跟着主题走 —— data URI
 * 里的十六进制是主题的死角,var() 进不去。 */
.theme-toggle {
    position: relative;
    width: 40px;
    height: 40px;
    padding: 0;
    border-radius: 10px;
    border: 1px solid var(--border);
    background-color: var(--surface);
    color: var(--fg);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background-color 0.15s ease;
}
.theme-toggle::before {
    content: "";
    display: block;
    width: 20px;
    height: 20px;
    background-color: currentColor;
    mask-repeat: no-repeat;
    mask-size: contain;
    mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M21 12.8A9 9 0 1111.2 3a7 7 0 009.8 9.8z'/%3E%3C/svg%3E");
}
.theme-toggle:hover { background-color: var(--hover); }
.theme-toggle:active { background-color: var(--active); }
```

- [ ] **Step 4: 两个 ☰ 改成顶栏里的常规流按钮**

`docs/typst/lib/css/outline.css:70-96` 整段替换。原规则用 `position: fixed; top: 16px` 加一个烤死 `stroke='%2319181f'` 的 data URI；新规则去掉定位、把图标挪到 `::before`（`☰` 字形已经被 `font-size: 0` 藏了，伪元素正好占那个位置）：

```css
/* 两个抽屉的开关。固定定位、不占 grid 格子的时代结束了 —— 它们现在在
 * .site-header 里走常规流,顶栏 sticky 保证它们滚动时依然够得着。
 * 图标用 mask + currentColor:data URI 里烤死的十六进制是主题死角,
 * var() 进不去,mask 才跟得住。 */
.toc-toggle, .outline-toggle {
    width: 40px;
    height: 40px;
    padding: 0;
    border-radius: 10px;
    background-color: var(--surface);
    border: 1px solid var(--border);
    color: var(--fg);
    font-size: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background-color .15s ease;
}
.toc-toggle::before, .outline-toggle::before {
    content: "";
    display: block;
    width: 20px;
    height: 20px;
    background-color: currentColor;
    mask-repeat: no-repeat;
    mask-size: contain;
    mask-position: center;
    mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round'%3E%3Cpath d='M4 7h16M4 12h16M4 17h16'/%3E%3C/svg%3E");
}
.toc-toggle:hover, .outline-toggle:hover { background-color: var(--hover); }
.toc-toggle:active, .outline-toggle:active { background-color: var(--active); }
```

接着删掉紧随其后的两条定位规则（:93-94）：

```css
.toc-toggle { left: 16px; }
.outline-toggle { right: 16px; }
```

- [ ] **Step 5: `main { padding-top: 48px }` 删掉**

`docs/typst/lib/css/outline.css:100`：

```css
main {
    padding-top: 48px;
}
```

整条删掉（`@media (min-width: 1080px)` 里那条 `main { padding-top: 0 }` 也一并删掉 —— 现在基础规则就是 0）。

- [ ] **Step 6: 五个偏移量改走 `--header-h`**

| 规则 | 改成 |
|---|---|
| `toc-component`（基础，窄屏抽屉） | `top: var(--header-h);` 取代 `top: 0;`；`max-height: calc(100vh - var(--header-h));` 取代 `max-height: 100vh;` |
| `.outline`（基础，窄屏抽屉） | 同上两条 |
| `.drawer-overlay` | `top: var(--header-h);` 取代 `top: 0;` |
| `.outline`（`@media (min-width: 768px)` 里的常驻列） | `top: calc(var(--header-h) + 16px);` 取代 `top: 32px;`；`height: calc(100vh - var(--header-h) - 32px);` 取代 `height: calc(100vh - 64px);` |
| `toc-component`（`@media (min-width: 1080px)` 里的常驻列） | `top: var(--header-h);` 取代 `top: 0;`；`height: calc(100vh - var(--header-h));` 取代 `height: 100vh;` |

- [ ] **Step 7: 构建**

```shell
cd F:/rust2genshin && export PATH="$PATH:/c/Users/53548/AppData/Local/Microsoft/WinGet/Links" && typst compile --root docs/typst ./docs/typst/main.typ target/debug/docs --features bundle,html --format bundle; echo "EXIT=$?"
```

预期 `EXIT=0`。

- [ ] **Step 8: 断言顶栏进了 HTML**

```shell
cd F:/rust2genshin && grep -o '<header class="site-header">' target/debug/docs/user/index.html && grep -c 'class="theme-toggle"' target/debug/docs/user/index.html
```

预期：第一行打印 `<header class="site-header">`；第二行输出 `1`。

- [ ] **Step 9: 三个断点各看一次**

打开 `target/debug/docs/user/index.html`，把浏览器宽度分别调到 **375px / 800px / 1280px**，每档确认：

- 顶栏在最上，两侧分别是 ☰ + 站名、主题按钮 + ☰。
- 375px：点左 ☰，左抽屉从顶栏下方滑入，遮罩也从顶栏下方开始盖住正文，不盖住顶栏。
- 375px：点右 ☰，右抽屉同上。
- 800px：右栏是常驻列，滚动时停在顶栏下面一行，不被顶栏压住；左 ☰ 仍是抽屉。
- 1280px：两栏都常驻，☰ 都消失，只剩站名和主题按钮。
- 滚动一段，正文从顶栏底下过去，顶栏停在原地；正文没有被顶栏压住（`main` 顶部不再有 48px 死白）。

- [ ] **Step 10: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/lib/outline.typ docs/typst/lib/css/outline.css && git commit -m "feat(docs): add sticky header bar with theme toggle"
```

---

### Task 6: 深色调色板校准

**Files:**
- Modify: `docs/typst/lib/css/theme.css`（`:root[data-theme="dark"]` 块）

Task 1 里的深色值是**提议值**，不是终值。本 task 校准它们。

- [ ] **Step 1: 在浏览器里打开深色模式**

开发者工具 → Rendering → Emulate CSS `prefers-color-scheme: dark`。或者直接点顶栏的主题按钮（此时系统是浅色，`theme.js` 会存下 `dark`）。

- [ ] **Step 2: 量对比度**

在 console 里跑，对每个「字 / 底」组合取实测值（`getComputedStyle` 拿的是最终解析值，比手算准）：

```js
function lum(c) {
  const [r, g, b] = c.match(/\d+(\.\d+)?/g).slice(0, 3).map(Number)
    .map(v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4 });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function ratio(fgEl, bgEl) {
  const a = lum(getComputedStyle(fgEl).color), b = lum(getComputedStyle(bgEl).backgroundColor);
  return ((Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)).toFixed(2);
}
ratio(document.body, document.body);                                    // 正文 ≥ 4.5
ratio(document.querySelector('.outline li.current > *:first-child'), document.querySelector('.outline'));  // 当前条目 ≥ 4.5
ratio(document.querySelector('main a'), document.querySelector('main')); // 链接 ≥ 4.5
```

- [ ] **Step 3: 按实测结果调整 `theme.css` 的深色值**

验收线：正文 / `.outline` / 链接 / 卡片文字 ≥ 4.5:1；`--accent`、边框 ≥ 3:1。任何一项不达标，调**深色那一侧**的值（不要动浅色块，浅色已经过 Step 3 的视觉比对），重跑 Step 2。

- [ ] **Step 4: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/lib/css/theme.css && git commit -m "fix(docs): calibrate dark palette contrast"
```

---

### Task 7: 补文档

**Files:**
- Modify: `docs/typst/dev/docs/layout.typ`

- [ ] **Step 1: 在 `layout.typ` 末尾加第四节**

文件末尾追加（`#set raw(lang: "css")` 在文件头部已经设过了，直接写）：

```typst
= 四、主题靠自定义属性跨 shadow 边界

四个组件 —— `toc-component`、`code-block`、`expand-panel`、`ui-graph` —— 都在自己的 shadow root 里渲染。页面样式选不进去；`style.css` 里写的 `.ui-graph` 规则一条都不会命中 shadow 内部的节点盒子。

能穿过去的只有*自定义属性*：`--bg` 挂在 `:root` 上，会一路继承进 shadow tree。所以组件 CSS 一律引用 `var(--x)`，颜色只在 `lib/css/theme.css` 的 `:root` 和 `:root[data-theme="dark"]` 两个块里各写一次。

两处例外，值得单独记：

- `var()` 在 `url("data:image/svg+xml,…")` 里*不生效*。图标 URI 里烤死的十六进制是主题死角。要跟着主题走，就把图标改成 `mask-image` + `currentColor`（`.toc-toggle` 就是这么改的）。
- `ui-graph` 的渲染样式是 `lib/js/graph.js` 里内联的 `<style>` 字符串，照样能用 `var()` —— shadow 边界挡住的是选择器，不是继承。

深色模式的主题由 `lib/js/theme.js` 在 `<head>` 里解析成 `documentElement` 上的 `data-theme`，所以它*依赖 JS*：没 JS 就只有浅色。这在本项目可接受 —— 侧栏、节点图、复制按钮本来就全是 JS 驱动的。
```

- [ ] **Step 2: 构建 + 看页面**

```shell
cd F:/rust2genshin && export PATH="$PATH:/c/Users/53548/AppData/Local/Microsoft/WinGet/Links" && typst compile --root docs/typst ./docs/typst/main.typ target/debug/docs --features bundle,html --format bundle; echo "EXIT=$?"
```

打开 `target/debug/docs/dev/docs/layout.html`，确认第四节渲染正常、目录树里出现新条目。

- [ ] **Step 3: 提交**

```shell
cd F:/rust2genshin && git add docs/typst/dev/docs/layout.typ && git commit -m "docs(docs): document theme variables and shadow boundaries"
```

---

### Task 8: 验收

**Files:** 无改动，只验证。

- [ ] **Step 1: 全量构建**

```shell
cd F:/rust2genshin && export PATH="$PATH:/c/Users/53548/AppData/Local/Microsoft/WinGet/Links" && typst compile --root docs/typst ./docs/typst/main.typ target/debug/docs --features bundle,html --format bundle; echo "EXIT=$?"
```

预期 `EXIT=0`。

- [ ] **Step 2: 两主题 × 三断点**

用 Playwright 或浏览器分别在 **375 / 800 / 1280px**、**浅色 / 深色**下过一遍这几个页面：

| 页面 | 覆盖到的组件 |
|---|---|
| `target/debug/docs/user/index.html` | 顶栏、两栏、目录、卡片 |
| `target/debug/docs/dev/index.html` | 代码块（shell / json）、行内 `code`、expand-panel |
| `target/debug/docs/arch/low/dataflow.html` | `ui-graph`：拖拽、缩放、reset、连线、引脚标签 |
| `target/debug/docs/arch/docs/index.html` | 三种 `ui-card` |
| `target/debug/docs/dev/debugging.html` | 长页面滚动，验证顶栏 sticky |

每档确认：没有亮色残片、没有不可读的文字、没有溢出或错位。

- [ ] **Step 3: 行为**

- 点主题按钮 → 页面立即切换，`aria-label` 变成目标状态（「切换到浅色模式」）。
- 刷新 → 保持上次的选择。
- 点站内链接跳转 → 保持。
- 开发者工具切系统主题：*没有*手动选过时，页面实时跟随；手动选过（按钮点过一次）时，**不**跟随。
- 深色系统下首次打开 → 不闪一下浅色。
- 无痕/禁 `localStorage` 下点按钮 → 页面正常切换，只是刷新后回到系统设置（不报错）。

- [ ] **Step 4: 回归 `dev/docs/layout.typ` 的三个坑**

- **`.block` 包装**：正文里块级内容仍应撑满，不因为新顶栏改变宽度。
- **全局 `line-height: 0.8`**：正文列表项没有上下叠字；目录树和侧栏行距正常。
- **`.block { width: fit-content }`**：顶栏那行没有因为宽度塌陷而挤成一团。

- [ ] **Step 5: 断言仓库里没有残留的颜色字面量**

```shell
cd F:/rust2genshin/docs/typst && grep -rnE '#[0-9a-fA-F]{3,8}\b' lib/css lib/js --include=*.css --include=*.js | grep -v 'lib/css/theme.css'
```

预期：无输出。data URI 里 `currentColor` 的不算（不是 hex 字面量），`code_block.css` 的 `black`/`white` 是关键字也不匹配这个正则。

- [ ] **Step 6: 提交（如果有改动）**

```shell
cd F:/rust2genshin && git status --short
```

只读检查；没有改动就不提交。

---

## Self-Review

**Spec 覆盖**

| Spec 要求 | 落在 |
|---|---|
| `:root` token 层，`theme.css` 两块 | Task 1 |
| `theme.js` head 阻塞脚本 + `localStorage` + `matchMedia` | Task 2 |
| 八个 CSS 文件字面量 → `var()` | Task 3 |
| `graph.js` shadow 内联 `<style>` → `var()` | Task 4 |
| `<header class="site-header">` + 站名 + 主题按钮 | Task 5 Step 1 |
| `grid-template-rows` + `grid-column: 1/-1` | Task 5 Step 2 |
| sticky 顶栏 | Task 5 Step 3 |
| `☰` 按钮改 `position: static` + `::before` mask | Task 5 Step 4 |
| `main { padding-top: 48px }` 删除 | Task 5 Step 5 |
| 五个偏移量改走 `--header-h` | Task 5 Step 6 |
| 深色值重新调而非调暗 | Task 1 + Task 6 校准 |
| `code-block[lang="output"]` 保持黑 | Task 3 例外清单 |
| `card.css` mask 不动 | Task 3 Step 6 |
| `layout.typ` 第四节 | Task 7 |
| 验收清单（两主题 × 三断点、行为、对比度、三个坑） | Task 8 |

**Placeholder 扫描**：无 TBD / TODO / 「同上」/「类似 Task N」。每处 CSS 都给了完整替换文本。

**命名一致性**：`--graph-box` / `--graph-border` / `--graph-edge` / `--graph-pin` / `--graph-canvas` / `--graph-reset-bg` 在 Task 1 定义、Task 3（`graph.css`）和 Task 4（`graph.js`）使用，名字一致。`.theme-toggle`、`--header-h`、`.site-header` 三处一致。

**已知的计划外风险**：`cargo run -p rust2genshin-docs` 在本机不可用（依赖 build script 崩溃）。若将来 cargo 恢复，`docs/src/compile.rs` 无需改动 —— 本计划只改 `typst/` 下的静态资源。