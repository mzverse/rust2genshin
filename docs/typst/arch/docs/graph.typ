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

节点自己又是一层：盒子里放一个无名 `<slot>`，把节点里的内容（标题文本连同一堆 `<pin>`）投影进来。引脚由 `::slotted(pin) { display: none }` 藏掉，剩下的就是标题 —— 于是盒子 `width: max-content` 量出来的宽度天然就是标题的宽，不用另外量一个 title 元素。

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

= 交互

左键拖节点，右键 / 中键拖整张图（空白处左键不动），滚轮缩放。

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

= 布局

生成时自动排一次，不布局就没法连线。分层是反复松弛 `rank[to] >= rank[from] + 1`，最多 `n` 轮 —— Bellman-Ford 的形状，能处理环（环里的边顶多让层数白涨一点，不会死循环）。列内再跑两轮重心排序减少交叉。

*整段是确定性的*：遍历顺序一律按文档顺序，平手按文档顺序（`.sort` 在 V8 上稳定），没有任何依赖对象键序的地方。文档里的示意图位置每次构建都一样，截图才对得上。