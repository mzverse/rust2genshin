#import "/lib/lib.typ": *;
#let title = [节点图];
#show: template;

#set raw(lang: "css");

文档站里的节点图不是图片、也不是一串手写坐标 —— 它是两个自定义元素：`<ui-graph>` 装一整张图，`<ui-graph-node>` 装一个节点。`lib/graph.typ` 生成它们的 light DOM（数据载体），渲染和交互全在 `lib/js/graph.js` 里 —— 样式也写在那儿（内联 `<style>`，因为 shadow root 够不着别处的样式表），`lib/css/graph.css` 只管 light DOM 侧。

写法（`graph` / `node` / `pin` / `connection` 怎么写）见#link("/dev/docs/graph.html")[节点图（开发栏）]。这一页讲*它怎么搭起来的*，以及每条结构为什么长这样 —— 大部分都对应一个踩过的坑。

= 数据在 light DOM，渲染在 shadow DOM

文档里写下的 `<ui-graph-node>` 和 `<connection>` 是*数据载体*：节点的标题就是投影进来的那段文本，引脚的 `dir` / `kind` / `type` / `label`、连接的两端编码在属性里。JS 读它们，在 shadow root 里造真正的图形。

引脚没有 id —— 同一节点里同 `kind` + 同 `dir` 的算一组，组内第几号在 `build()` 里数出来，拼成 `data-in-1` 这样的名字。连接里写的也是组内序号，两端的 `dir` 各数各的。这个映射要等*所有*节点收齐才能查，所以「连接引用了不存在的节点」「序号越界」只能构建完再报（`console.error`）；`graph.typ` 那边能在构建期 `assert` 的，只有引脚和连接*自身*写错那类。

自定义元素本身也要用 `custom_element` 注册一次（`lib/graph.typ` 里的 `graph_element`），把脚本和样式一起挂上。这一套是全站通用的，见#link("/arch/docs/custom-element.html")[自定义元素]。

这里有个解析顺序的坑：脚本在 `<head>`，自定义元素在解析到开标签的那一刻就触发 `connectedCallback`，此刻它自己的子节点还没进 light DOM。所以两边都推迟到文档就绪再建（`document.readyState === 'loading'` 那段判断），而不能一上来就 `querySelectorAll`。

= 一层 svg，一个变换

shadow root 里只有一层 `<svg class="g-layer">`，*没有 `viewBox`* —— 挂了 `viewBox` 就得考虑 `meet` 的留白换算，坐标和实际像素之间隔了一层。不挂就没有这一层：用户坐标就是 px。

连线和节点都在这一层里，整层只挂一个 `--g-tf`（`translate(tx,ty) scale(s)`）变换，平移缩放一次带走。节点的位置是 `<foreignObject>` 的 `x` / `y` 属性，不自己带变换 —— 两拨东西各写一遍变换再对齐，迟早会对不齐。

连线是 `<path>`，*先建*；节点 `<foreignObject>` 后挂在同一个 svg 下。SVG 的绘制顺序就是文档顺序，所以节点自然压在线上，交叉处看得清是谁穿过谁。

`overflow: visible` 开在这层 svg 上，让拖到图外的节点还能露出来；真正裁剪的视口是外面那个 `.graph-root`（`overflow: hidden`）。

= 节点是 foreignObject + slot

节点*不搬家*：shadow root 里放一个 `<slot name="节点id">`，把文档里写的 `<ui-graph-node>` 投影进去。元素仍然在 light DOM 里，所以 `ui-graph > ui-graph-node` 这类选择器照样命中 —— 节点的光标就是这么做的（shadow root 里的样式够不着 light DOM 的节点）。节点没给 `id` 时（只是单独摆一个看看形状），JS 按文档顺序编个名字当 slot 名，且不进 `byId` —— 连接也就引用不到它。

节点自己又是一层：盒子里放一个无名 `<slot>`，只接标题那段文本节点 —— 每根 pin 都带了 `slot` 名，被各自的标签 `<slot>` 认领去了。于是盒子 `width: max-content` 量出来的宽度天然就是标题的宽，不用另外量一个 title 元素。

引脚标签是同一套投影的另一半：JS 给 light DOM 里的每个 pin 写上 `slot` 属性（值就是那套组内序号名字），shadow 里每个标签是一个 `<foreignObject>` 包一个 `<div>`，div 里放 `<slot name="...">` 认领对应的 pin。尺寸量 div 的 `getBoundingClientRect`，而不是估字宽 —— 页面的字体、行高、文字样式怎么改都跟着变。foreignObject 只当裁剪框，宽高跟着量出来的 div 写。

这里有两个坑：`<slot>` 的选择属性叫 `name`，写成 `slot=` 等于把这个 slot 自己变成可分配的节点，它就不再参与分配、`assignedNodes()` 恒空；slot 嵌在 foreignObject 里没关系，分配只看 shadow root。另外 slot 的分配是结构性的、绘制看 `display` —— `graph.css` 里那条 `ui-graph pin { display: none }` 得去掉，否则投影出来的标签也被一起藏掉。没有标签的引脚（`slot` 名找不到对应 `<slot>`）本来就不渲染，不用额外处理。

=== 顺序：先 slot，再量尺寸

有 shadow root 的宿主*不渲染没被 slot 分走的子节点*。所以 `ui-graph.#build` 里必须先把 `foreignObject` + `slot` 建好（尺寸给个占位值），再 `build()` + `relayout()` 把量出来的真实尺寸写回 `foreignObject`。

反过来写的话，节点宽度会全塌成最小值、标题被裁成 `SetLocal<in` —— 而构建全程不报错，很难查。

=== 引脚：形状在盒子边界上，元素比盒子宽

引脚的圆点（数据流）和三角（控制流）以*盒子边界*为中心 —— 一半在盒里、一半在盒外。

`<foreignObject>` 按自己的矩形*硬裁剪*，所以节点元素得比盒子宽出左右各一圈 `BLEED`（`CTRL_R + 1`，7px），盒子缩在 `:host` 的 `padding` 里：

```css
:host { box-sizing: content-box; padding: 0 7px; }
```

`box-sizing` 是 `content-box`，所以 JS 里 `style.width` 设的仍是*盒子*宽，元素实际宽度自动多出 `2 * BLEED`。`.g-shapes` 那层 svg 和 `foreignObject` 的宽度都加上这个量。

引脚坐标因此落在 `BLEED` 和 `BLEED + w` 上（正好是盒子左右边界），`worldAnchor` 就是 `this.x + p.x`，不用再按方向加宽度。

宽度只按引脚朝盒内那半截算：按*行*统计，同一行的入口和出口在同一个 `y` 上会撞，所以中间留一道 `MID_GAP`；标签从边界再让开半个形状宽加 `LABEL_GAP`。

连线*不画箭头* —— 端点就是引脚中心，再补一个三角只是多余。

=== 盖在上面的形状层要点名让路

引脚形状那层 svg（`.g-shapes`）是绝对定位铺满整个节点的，又排在节点的 `foreignObject` 之后 —— SVG 的绘制顺序就是文档顺序，于是它压在标题上面。标题里要是写了链接（`#link(...)`），点下去命中的是 `<svg>` 而不是那个 `<a>`，链接彻底点不动：合成事件里 `composedPath()[0]` 直接是 `svg`。

所以这层要 `pointer-events: none`，再用 `.g-shapes > circle, .g-shapes > polygon` 把引脚形状本身放回来 —— 它们得能悬停出类型提示。不能靠调 DOM 顺序把 svg 挪到 `foreignObject` 前面：引脚圆点有一半在盒子外面，挪到底下会被 `.g-box` 的白底盖住。

顺带一个连带效应：`pointer-events` 会*继承*，而引脚标签的 `foreignObject` 也挂在这层 svg 下，所以 `.g-label` 得显式写 `pointer-events: auto`，否则标签里的链接（`ctrl_out[#link(...)[]]` 这种）同样点不动 —— 而且它的表现是「命中 `.g-label` 那个 div」，比标题那种更隐蔽。

= 交互

桌面：左键拖节点，右键 / 中键拖整张图，空白处左键什么都不做，滚轮缩放。触摸：单指按在节点上拖节点，按在空白处拖整张图 —— 触摸没有右键，空白处是唯一能搬图的地方；鼠标那边则保持「左键只管节点」，两条路按 `pointerType` 分开。

两份起点：世界坐标（拖节点用）和屏幕坐标（平移用）。平移吃*屏幕像素*，图跟手、跟缩放无关；拖节点吃*世界坐标*，图缩放多少，指针走过的世界距离就是多少。两者混用的话，缩放之后越拖越飞。

缩放锚在指针下的那个点：先记下它的世界坐标，缩放之后把平移调回去，让同一个世界点仍落在指针下面。

== 光标：捕获元素说了算

`pointerdown` 里有一句 `root.setPointerCapture(ev.pointerId)` —— 没有它，指针一拖出 svg 事件就断了。但捕获期间浏览器按*捕获元素*的光标显示，不再看指针底下实际是哪个元素。

所以 light DOM 里那条

```css
ui-graph.dragging > ui-graph-node { cursor: grabbing; }
```

只管住「按下但还没移动」那一段，一移动就切到 `.graph-root` 的 `cursor: auto`，光标弹回箭头。shadow 内必须再写一份：

```css
.graph-root.dragging { cursor: grabbing; }
```

平移那边的 `.graph-root.panning` 是同一个原因，不是顺手加的。

=== 拖动不能吃掉交互元素

`pointerdown` 里有 `setPointerCapture` 和 `ev.preventDefault()`，两者都会让后续的 `pointerup` 和 `click` 落到 `.graph-root` 身上 —— 重置按钮和节点标题里的链接就再也收不到 `click` 了（表现是按钮按下去什么反应都没有，链接点了不跳）。所以 `pointerdown` 开头先看一眼目标：`button, a, input, select, textarea` 直接放行，不捕获、不拦截。

另外建按钮要用 `document.createElement` 而不是文件里的 `el()` —— `el()` 一律建 SVG 名字空间的元素，HTML 元素那样建出来没有盒子（`getBoundingClientRect()` 全 0），按下去也未必是它。

= 布局

生成时自动排一次，不布局就没法连线。分层是反复松弛 `rank[to] >= rank[from] + 1`，最多 `n` 轮 —— Bellman-Ford 的形状，能处理环（环里的边顶多让层数白涨一点，不会死循环）。列内再跑两轮重心排序减少交叉。

*整段是确定性的*：遍历顺序一律按文档顺序，平手按文档顺序（`.sort` 在 V8 上稳定），没有任何依赖对象键序的地方。文档里的示意图位置每次构建都一样，截图才对得上。