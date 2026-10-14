/* 简单的节点图。
 *
 * light DOM 里的 <ui-graph-node> 和 <connection> 是数据载体(graph.typ 生成)。
 * 节点就是文档里写的元素本身 —— JS 只量尺寸、定位置,不搬家。
 *
 * shadow root 里只有一层:一个 <svg>,尺寸等于整张图,没有 viewBox ——
 * 用户坐标就是 px,坐标换算不用管 meet 的留白。连线和节点都在这一层里:
 *   - 连线是 <path>,先画。
 *   - 每个节点是一个 <foreignObject>,矩形就是节点元素的矩形,里面靠 <slot>
 *     把文档里写的 <ui-graph-node> 投影进来(元素不搬家,还在 light DOM,
 *     页面样式照样管得到)。
 * 整层只挂一个 `--g-tf` 变换,平移缩放一次带走,两拨东西不会对不齐。
 *
 * foreignObject 按自己的矩形硬裁剪,而引脚形状是要伸出盒子边界的 —— 所以
 * 节点元素本身比盒子大出一圈 `BLEED`(见 GraphNode.relayout),把伸出去的
 * 那半截也圈进矩形里,裁不到。
 *
 * 布局是**确定性**的:同一份文档每次生成的位置完全一样 —— 遍历顺序一律按
 * 文档顺序,列内排序的平手按文档顺序,没有任何依赖对象键序的地方。文档里的
 * 示意图位置每次构建都一样,截图才对得上。
 *
 * 交互:左键拖节点,右键 / 中键拖整张图,滚轮缩放。左键在空白处什么都不做。 */

const SVG_NS = 'http://www.w3.org/2000/svg';
const XHTML_NS = 'http://www.w3.org/1999/xhtml';

// 布局常量,单位 px(缩放前的世界坐标)。
const NODE_MIN_W = 96;
const NODE_PAD_X = 12;   // 标题左右留白
const NODE_HEAD_H = 30;  // 顶部标题条
const NODE_HEAD_PAD = 5; // 标题离盒子上沿
const PIN_GAP = 20;      // 每根引脚占的高度
const PIN_TOP = 10;      // 标题条到第一根引脚的距离
const PIN_BOT = 10;      // 最后一根引脚到节点底边的距离
const PIN_R = 5;         // 数据流圆点半径
const CTRL_R = 6;        // 控制流三角形的半宽
// 引脚形状以盒子边界为中心向两侧伸:伸得最远的是控制流三角的顶点,离边界
// CTRL_R;数据流圆点加上描边是 PIN_R + 0.75。节点元素因此要比盒子宽出左右
// 各这么一圈,把伸出去的半截也圈进自己的矩形(见 GraphNode.relayout)。
// 多给 1px 是给描边的抗锯齿留缝。
const BLEED = CTRL_R + 1;
const COL_GAP = 46;      // 列间距
const ROW_GAP = 14;      // 同一列里节点间距
const PAD = 12;          // 整张图四周留白

const TITLE_PX = 14;
const PIN_PX = 11;

// 引脚形状到标签的空,以及同一行左右两个标签之间至少留的缝。
const LABEL_GAP = 4;
const MID_GAP = 12;

// 量尺寸之前,节点那个 <foreignObject> 先按这个尺寸支起来(见 ui-graph.#build)。
const FO_PROBE_W = 400;
const FO_PROBE_H = 200;

function el(tag, attrs, parent) {
    const n = document.createElementNS(SVG_NS, tag);
    for (const k in attrs) n.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(n);
    return n;
}

/** 文档就绪后跑一次。
 *
 * 脚本在 <head>,自定义元素在解析到开标签的那一刻就 upgrade,此刻它自己的子
 * 节点(pin、connection)还没进 light DOM —— 现在读只会读到空的,而且不报错。
 * 两个自定义元素都得推迟,所以这个判断写一份就好。 */
function whenReady(fn) {
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', fn, { once: true });
    } else {
        fn();
    }
}

/* shadow 内部的样式。每张节点一份(写在各自 shadow 里,页面样式进不来),
 * 所以内容只建一次、到处复用。
 *
 * 元素左右各让出 BLEED 的 padding,.g-box 缩在圈里面,引脚形状才伸得出去。
 * 宽窄由 .g-box 的 min-width 定(见 relayout)。
 *
 * .g-box 的 width: max-content 让它自己收到标题那么宽;min-width 由 JS 按
 * 引脚排布算好后写回来。标题就是 slot 投影进来的那个文本节点,::slotted 只管
 * 元素、管不到文本 —— 所以字形得写在盒子上。 */
const NODE_CSS = `
    :host { display: block; position: relative; box-sizing: content-box;
        padding: 0 ${BLEED}px; }
    .g-box { position: relative; width: max-content; min-width: ${NODE_MIN_W}px;
        height: 100%; box-sizing: border-box; background: #fff;
        border: 1.5px solid #c9cbd4; border-radius: 7px; overflow: hidden;
        font-family: inherit; font-size: ${TITLE_PX}px; font-weight: 600;
        line-height: 21px; color: #19181f; white-space: nowrap;
        /* 标题是投影进来的裸文本节点,没法单独给它设 margin,靠盒子的上内边距
         * 把它压到标题条中间去。 */
        padding: ${NODE_HEAD_PAD}px ${NODE_PAD_X}px 0; }
    :host(:hover) .g-box { border-color: #11566c; }
    :host(.hot) .g-box { border-color: #11566c; border-width: 2.5px; }
    :host(.dim) { opacity: .28; }
    /* 形状这层 svg 盖在整个节点上,又排在 foreignObject 之后(绘制顺序就是
     * 文档顺序),默认会把标题里的链接整个吃掉 —— 点下去 path0 是 <svg>,
     * 不是那个 <a>。所以这层不接指针,只把引脚形状本身放回来:它们要悬停
     * 出类型提示,而它们又在盒子边界上、压在 foreignObject 上,不能靠改
     * DOM 顺序挪到底下去(那样露在盒子外面的那半截会被白底盖住)。 */
    .g-shapes { position: absolute; left: 0; top: 0; pointer-events: none; }
    .g-shapes > circle, .g-shapes > polygon { pointer-events: auto; }
    /* 引脚标签。走 foreignObject 包一个 div,尺寸量 div 的 rect —— 页面的字体、
     * 行高、文字样式随便怎么改都跟着变,不用拿 SVG 文本的 getComputedTextLength
     * 估。div 自己 max-content 撑开,fo 只负责把它量到的那块地方裁出来。 */
    /* 标签的 fo 也是挂在 .g-shapes 下的,所以上面那条 pointer-events: none
     * 会顺着继承下来 —— 投影进来的引脚文本(含里面的链接)就点不动了。这里
     * 显式开回来。 */
    .g-label { font-family: inherit; font-size: ${PIN_PX}px; line-height: 1.2;
        color: #7a7c86; white-space: nowrap; width: max-content;
        pointer-events: auto; }
    .g-ctrl { fill: #11566c; }
    .g-data { fill: #fff; stroke: #11566c; stroke-width: 1.5; }
`;

/* ------------------------------------------------------------------ *
 * ui-graph-node —— 一个节点
 * ------------------------------------------------------------------ */

/**
 * 节点元素。盒子就是 `<div class="g-box">` —— 它同时是 border、背景和尺寸
 * 的唯一来源。`<slot>` 留着投影 light DOM 里额外写的内容。
 *
 * 引脚的圆点/三角不在 div 里,而在一个盖在盒子上的 `<svg>` 里。形状以盒子
 * 边界为中心 —— 一半在盒里、一半在盒外。所以**元素**比盒子宽:左右各让出
 * 一圈 `BLEED`,`.g-box` 就缩在这一圈里面。图里节点是拿 `<foreignObject>`
 * 渲染的,它按自己的矩形硬裁剪 —— 圈进去了就裁不到。
 *
 * 外部画连线要引脚坐标,问它要:
 *   `worldAnchor(id)` —— 引脚形状中心,也就是盒子边界上那个点(世界坐标);
 *     id 是组内序号那套名字(`data-in`、`data-in-1`…)。坐标在 `relayout()`
 *     里算好存进 `points`,所以拖动节点时相对坐标不变,连线只要把元素位置
 *     加上去。
 */
class GraphNode extends HTMLElement {
    connectedCallback() {
        // 解析早于子节点,得等文档就绪(见 whenReady)。ui-graph 也会主动调
        // build(),所以幂等。
        whenReady(() => this.build());
    }

    /** 建 shadow:盒子 + 引脚形状。幂等 —— 早于子节点解析时是空跑,等
     * ui-graph 在自己那次 build 里再调一次。 */
    build() {
        if (this.built) return;
        // 解析中:子节点(pin)还没进来,现在建等于建个空盒子。等 ui-graph
        // 在文档就绪后回头调这一下。
        if (document.readyState === 'loading') return;
        this.built = true;

        this.#readPins();
        this.#buildShadow();

        this.addEventListener('pointerenter', () =>
            this.dispatchEvent(new CustomEvent('graph-hover', { detail: { id: this.id, on: true } })));
        this.addEventListener('pointerleave', () =>
            this.dispatchEvent(new CustomEvent('graph-hover', { detail: { id: this.id, on: false } })));

        // 自己量一遍。在图里 ui-graph 也会调,重复调没副作用;不在图里
        // (单独摆一个节点)就靠这一下。
        this.relayout();
    }

    /** 读 light DOM 里的 <pin>,定好组内序号和行号。
     *
     * 两个「号」别混:
     *   `id`  = 组内序号那套名字(`data-in-0`),连接靠它引用,也拿来当 pin 的
     *           slot 名;
     *   `row` = 上下排第几行(控制流排在数据流上面),定 y 用。
     * 之前行号也叫 slot,和 `p.el.slot` 那个 DOM 属性同名,读代码时极易看错。
     */
    #readPins() {
        const pins = [...this.querySelectorAll('pin')].map(el => ({
            el,
            dir: el.getAttribute('data-dir'),
            kind: el.getAttribute('data-kind'),
            // 名字写在 pin 的内容里,靠 slot 投影出来;类型只在悬停时浮出来,
            // 平时不占地方。
            label: (el.textContent || '').trim(),
            type: el.getAttribute('data-type') || '',
        }));
        const counts = new Map();
        for (const p of pins) {
            const g = `${p.kind}-${p.dir}`;
            const i = counts.get(g) || 0;
            // 别省 0:连接里两端都写全序号,这里也一律带,两边的规则要一样。
            p.id = `${g}-${i}`;
            counts.set(g, i + 1);
            // 用 slot 属性把这个 pin 指给 shadow 里对应的 <slot> —— 标签就是
            // 这么投影出来的。认领走 slot 名,所以名字必须和这里的 id 一致。
            p.el.slot = p.id;
        }
        // 行号按边分:控制流排在数据流上面(viewer 也是这个次序,看着顺)。
        for (const side of ['in', 'out']) {
            pins
                .filter(p => p.dir === side)
                .sort((a, b) => (a.kind === 'ctrl' ? 0 : 1) - (b.kind === 'ctrl' ? 0 : 1))
                .forEach((p, i) => { p.row = i; });
        }
        this.pins = pins;
        this.points = new Map();
    }

    /** 建 shadow:盒子 + 引脚形状 + 标签。 */
    #buildShadow() {
        const shadow = this.attachShadow({ mode: 'open' });
        const style = document.createElement('style');
        style.textContent = NODE_CSS;
        shadow.appendChild(style);

        // 盒子里只有一个无名 <slot>,只接标题那段文本节点 —— 每根 pin 都带了
        // slot 名,被各自的标签 <slot> 认领去了,不会再落到这儿。
        this.box = document.createElement('div');
        this.box.className = 'g-box';
        this.box.appendChild(document.createElement('slot'));
        shadow.appendChild(this.box);

        this.svg = el('svg', { class: 'g-shapes' }, shadow);

        for (const p of this.pins) {
            // 控制流:实心三角,以盒子边界为中心,尖端一律朝右 —— 顺着数据流
            // 的方向。入口朝左、出口朝右会让控制流读成"双向",看着像既有去
            // 又有回。入口/出口靠**位置**(左边缘/右边缘)区分,不靠朝向。
            if (p.kind === 'ctrl') {
                p.shape = el('polygon', { class: 'g-ctrl' }, this.svg);
            } else {
                p.shape = el('circle', { class: 'g-data', r: PIN_R }, this.svg);
                // 类型只在悬停这个圆点时浮出来。
                if (p.type) el('title', {}, p.shape).textContent = p.type;
            }
            if (p.label) {
                // 标签不自己造文本,而是把 light DOM 里那个 <pin> 的内容
                // 投影进来 —— pin 在 #readPins 里被指了 slot 名,这里按同一个
                // 名找它。排版和尺寸都由那个 div 说了算。
                p.fo = el('foreignObject', {}, this.svg);
                // foreignObject 里是 XHTML,得按 XHTML 的命名空间建 —— 用
                // document.createElement 建出来的是 HTML 节点,塞进去虽然
                // 大多时候也能显示,但规范上不对。
                p.labelEl = document.createElementNS(XHTML_NS, 'div');
                p.labelEl.className = 'g-label';
                // slot 的选择属性叫 name —— 给它写 slot= 是把**这个 slot 自己**
                // 变成可分配的节点,它反而就不再参与分配,assignedNodes 恒空。
                // 嵌在 foreignObject 里没关系,分配只看 shadow root。
                const s = document.createElementNS(XHTML_NS, 'slot');
                s.setAttribute('name', p.id);
                p.labelEl.appendChild(s);
                p.fo.appendChild(p.labelEl);
            }
        }
    }

    /**
     * 量出盒子尺寸、算出每根引脚相对元素左上角的坐标,存进 this.points。
     *
     * 标题就是 slot 投影进来的那段文本,宽度量盒子的内容宽;引脚标签量包住
     * 它的那个 div。两边都是量出来的,不是按字宽估算的 —— 长名字不会把节点
     * 挤窄。前提是节点已在文档里且可见(display:none 时量出来是 0)。
     */
    relayout() {
        // 形状以盒子边界为中心:圆心/三角心正落在边界上。往盒子**里面**伸的
        // 那半截 + 到标签的空 + 标签,就是从边界起算一根引脚占的宽 —— 排版只
        // 关心朝里这半截,朝外那半截由元素让出的 BLEED 兜着。
        const half = p => (p.kind === 'ctrl' ? CTRL_R : PIN_R + 1);

        // 量 div 而不是估字宽。div 自己是 max-content,不受 foreignObject 宽度
        // 约束(fo 只裁剪绘制),所以量得到的是标签本来的宽。量一次存下来 ——
        // 下面两处都要用,getBoundingClientRect 会触发强制重排。
        for (const p of this.pins) {
            const r = p.labelEl && p.labelEl.getBoundingClientRect();
            p.labelW = r ? Math.ceil(r.width) : 0;
            p.labelH = r ? Math.ceil(r.height) : 0;
        }
        const extent = p => half(p) + LABEL_GAP + p.labelW;

        this.rows = Math.max(1, ...this.pins.map(p => p.row + 1));
        // 左右各按行量:同一行的入口和出口在同一个 y 上,会撞。
        const rowIn = new Array(this.rows).fill(0);
        const rowOut = new Array(this.rows).fill(0);
        for (const p of this.pins) {
            const row = p.dir === 'in' ? rowIn : rowOut;
            row[p.row] = Math.max(row[p.row], extent(p));
        }

        // .g-box 是 border-box,量到的 rect 就是盒子外宽。this.w 一律指外宽,
        // 引脚坐标直接落在 BLEED / BLEED + w 上。
        //
        // 但 min-width 是上一轮 relayout 写回去的,直接量等于把上轮的结果当
        // 输入 —— 宽度会变成只增不减的历史累积,量不到标题真正的自然宽。量之前
        // 把它摘掉,量完再写回这轮算出来的值。
        const prevMin = this.box.style.minWidth;
        this.box.style.minWidth = '0px';
        const titleW = this.box.getBoundingClientRect().width;
        this.box.style.minWidth = prevMin;

        let w = Math.max(NODE_MIN_W, Math.ceil(titleW));
        for (let i = 0; i < this.rows; i++) {
            // 两边都有东西时中间再留一道缝,不然两个标签会贴上。
            w = Math.max(w, rowIn[i] + rowOut[i] + (rowIn[i] && rowOut[i] ? MID_GAP : 0));
        }
        this.w = Math.ceil(w);
        this.h = NODE_HEAD_H + PIN_TOP + this.rows * PIN_GAP + PIN_BOT;

        // 盒子是 max-content,靠 min-width 撑到算出来的宽;元素宽度是盒子外宽
        // 再加左右各一圈 BLEED(:host 的 padding)。
        this.box.style.minWidth = `${this.w}px`;
        this.style.width = `${this.w}px`;
        this.style.height = `${this.h}px`;
        // svg 盖住整个元素,连伸出去的那半截引脚一起。
        this.svg.setAttribute('width', this.w + 2 * BLEED);
        this.svg.setAttribute('height', this.h);
        // 在图里时,节点的矩形是那个 <foreignObject> 的,尺寸跟着量出来的走 ——
        // 一样要带上 BLEED,不然伸出去的形状正好被它裁掉。
        if (this.fo) {
            this.fo.setAttribute('width', this.w + 2 * BLEED);
            this.fo.setAttribute('height', this.h);
        }

        for (const p of this.pins) {
            const hw = half(p);
            // 引脚坐标是相对**元素**左上角的;盒子从 BLEED 开始,所以边界正好
            // 落在 BLEED / BLEED + w 上。
            p.x = p.dir === 'in' ? BLEED : BLEED + this.w;
            p.y = NODE_HEAD_H + PIN_TOP + p.row * PIN_GAP + PIN_GAP / 2;
            if (p.kind === 'ctrl') {
                p.shape.setAttribute('points', [
                    `${p.x + CTRL_R},${p.y}`,
                    `${p.x - CTRL_R},${p.y - CTRL_R}`,
                    `${p.x - CTRL_R},${p.y + CTRL_R}`,
                ].join(' '));
            } else {
                p.shape.setAttribute('cx', p.x);
                p.shape.setAttribute('cy', p.y);
            }
            if (p.fo) {
                // 标签以引脚中心为基准:入口往右让开、左边缘对齐,出口往左让开、
                // 右边缘贴住 —— 和 SVG 的 text-anchor 是一回事。fo 只是个裁剪框,
                // 尺寸跟着量出来的 div 写。
                p.fo.setAttribute('width', p.labelW);
                p.fo.setAttribute('height', p.labelH);
                p.fo.setAttribute('x',
                    p.dir === 'in' ? p.x + hw + LABEL_GAP : p.x - hw - LABEL_GAP - p.labelW);
                p.fo.setAttribute('y', p.y - p.labelH / 2);
            }
            this.points.set(p.id, p);
        }
    }

    /** 这根引脚在不在。 */
    hasPin(id) {
        return this.points.has(id);
    }

    /** 把节点摆到世界坐标 (x, y)。矩形就是那个 <foreignObject> 的 x/y ——
     * 节点层在 svg 里,跟着 svg 上那个 --g-tf 一起平移缩放,自己不用再带
     * 变换。 */
    setPosition(x, y) {
        this.x = x;
        this.y = y;
        if (this.fo) {
            this.fo.setAttribute('x', x);
            this.fo.setAttribute('y', y);
        }
    }

    /** 连线接在哪儿:引脚形状中心,也就是盒子边界上那个点(世界坐标)。 */
    worldAnchor(id) {
        const p = this.points.get(id);
        return p && { x: this.x + p.x, y: this.y + p.y };
    }

    dim(on) {
        this.classList.toggle('dim', on);
    }

    markHot(on) {
        this.classList.toggle('hot', on);
    }
}

customElements.define('ui-graph-node', GraphNode);

/* ------------------------------------------------------------------ *
 * ui-graph —— 整张图
 * ------------------------------------------------------------------ */

/** 三次贝塞尔:两端水平切线,控制点按水平距离取一段固定偏移。 */
function edgePath(x1, y1, x2, y2) {
    const dx = Math.max(30, Math.min(90, Math.abs(x2 - x1) * 0.5));
    // 往回指的边(x2 < x1)拉得更开,不然线会绕回节点肚子里
    const c = dx * (x2 >= x1 ? 1 : 2);
    return `M ${x1} ${y1} C ${x1 + c} ${y1}, ${x2 - c} ${y2}, ${x2} ${y2}`;
}

customElements.define('ui-graph', class extends HTMLElement {
    connectedCallback() {
        // 脚本在 <head>,自定义元素先于自己的子节点被解析(见 whenReady)。
        // `done` 挡住后续的重复调用。
        if (this.done) return;
        this.done = true;
        whenReady(() => this.#build());
    }

    #build() {
        this.nodes = [];
        this.edges = [];
        this.byId = new Map();
        // 元素 → 节点。没 id 的节点不进 byId,拖动时按 id 找不到,靠这张表。
        this.byEl = new Map();
        // 每个节点连着哪些边。悬停高亮和拖动重画都按邻接走,不然一个节点
        // 得扫一遍全部边。
        this.adj = new Map();

        // 节点就是文档里写的那些 <ui-graph-node> —— id 直接用 HTML 的 id。
        // id 可以不给(单独摆一个看看时不必费劲起名),那种节点不进 byId,
        // 连接自然也就引用不到它。
        const els = [...this.querySelectorAll('ui-graph-node')];
        const used = new Set();
        for (const n of els) {
            if (!n.id) continue;
            if (used.has(n.id)) {
                console.error(`graph: 节点 id 重复:${n.id}`, n);
            }
            used.add(n.id);
            this.byId.set(n.id, n);
        }
        // <slot name> 靠元素的 slot 属性认领。没 id 的节点按文档顺序补个名字
        // —— 得避开真 id 已经占掉的名字,不然两条抢同一个 slot,先来的那条直接
        // 渲染不出来(而且不报错)。改属性不改结构,元素还在原地。
        let anon = 0;
        for (const n of els) {
            let name = n.id;
            while (!name || used.has(name)) name = `n${anon++}`;
            used.add(name);
            n.slot = name;
            this.nodes.push(n);
            this.byEl.set(n, n);
            this.adj.set(n, []);
        }

        const shadow = this.attachShadow({ mode: 'open' });
        const root = document.createElement('div');
        root.className = 'graph-root';
        const style = document.createElement('style');
        style.textContent = `
            .graph-root { position: absolute; inset: 0; touch-action: none;
                user-select: none; -webkit-user-select: none; overflow: hidden; }
            /* 整张图只有这一层,坐标就是世界坐标:没有 viewBox,所以用户单位
             * 就是 px,坐标换算不用管 meet 的留白。连线和节点都在里面 ——
             * 一个变换带着走,不用两边各写一遍再对齐。
             * overflow: visible 让拖到图外面的节点还能露;真正裁剪的是
             * .graph-root 那个视口。 */
            .g-layer { position: absolute; inset: 0; width: 100%; height: 100%;
                overflow: visible; transform-origin: 0 0;
                transform: var(--g-tf, translate(0px, 0px)); }
            /* 每个节点一个 <foreignObject>,按自己的矩形硬裁剪。内容靠 <slot>
             * 把文档里写的 <ui-graph-node> 投影进来 —— 元素不搬家,还在 light
             * DOM,页面样式照样管得到(光标就靠这个)。 */
            foreignObject.g-node { overflow: hidden; }
            /* 平移整张图:右键 / 中键,以及触摸时空白处的单指。手型而不是十字 ——
             * 一路手型,按下去和平时长得一样。鼠标的空白处左键不拖,光标就
             * 还是默认的。 */
            .graph-root.panning { cursor: grab; }
            .graph-root.panning:active { cursor: grabbing; }
            /* 拖节点:按下时 setPointerCapture 把指针收在 .graph-root 上,
             * 浏览器这时按**捕获元素**的光标显示,不再看指针底下的节点 ——
             * light DOM 那条 grabbing 就够不着了,一开拖光标会弹回箭头。
             * 所以这里照着 .panning 再写一份。 */
            .graph-root.dragging { cursor: grabbing; }
            .g-edge { fill: none; stroke: #11566c; stroke-width: 1.6; stroke-linecap: round; }
            .g-edge-hit { fill: none; stroke: transparent; stroke-width: 10; }
            .g-edge.data { stroke-dasharray: 5 4; }
            .g-edge.dim { opacity: .12; }
            .g-edge.hot { stroke-width: 2.4; }
            /* 重置按钮。挂在 .graph-root 上,不进 .g-layer,所以不跟着缩放
             * 平移走 —— 一直待在视口右上角,按的就是"忘掉我拖过的一切"。 */
            .g-reset { position: absolute; top: 6px; right: 6px; z-index: 1;
                width: 22px; height: 22px; padding: 0; border-radius: 6px;
                border: 1px solid #c9cbd4; background: rgba(255, 255, 255, .9);
                color: #11566c; font-size: 14px; line-height: 1; cursor: pointer; }
            .g-reset:hover { background: #fff; }
        `;
        shadow.appendChild(style);
        shadow.appendChild(root);
        this.root = root;

        this.#render(root);
        this.#bindDrag(root);
    }

    /** 整个影子树重算一遍:丢掉拖出来的位置和缩放平移,回到自动布局。
     *
     * 节点自己的 shadow root 不用重建(它只管自己那点引脚),重挂
     * foreignObject + slot 让它们重新被投影、重新量尺寸就够了。 */
    #rebuild() {
        this.edges = [];
        for (const n of this.nodes) {
            this.adj.set(n, []);
            // 悬停留下的高亮 / 淡出状态也一起清掉,不然重算完还是花的。
            n.dim(false);
            n.markHot(false);
        }
        this.root.replaceChildren();
        this.view.reset();
        this.#render(this.root);
    }

    #render(root) {
        const svg = el('svg', { class: 'g-layer' }, root);
        // 连线先建,节点 <foreignObject> 后挂在同一个 svg 下 —— 文档顺序就是
        // 叠放顺序:节点压在线上,交叉处看得清是谁穿过谁。
        const edgeLayer = el('g', { class: 'g-edges' }, svg);

        const h = this.getAttribute('data-height');
        if (h) this.style.height = h;
        if (!this.nodes.length) {
            this.querySelector('.graph-fallback')?.remove();
            return;
        }

        // 每个节点一个 <foreignObject>,矩形就是节点元素的矩形 —— 比盒子宽出
        // 左右各一圈 BLEED,那半截伸出去的引脚形状也在里面(relayout 保证),
        // 所以既裁不到形状,又拦得住真越界的内容,不会糊到邻居身上。内容是投影
        // 进来的 <ui-graph-node> —— 元素不搬家,还在文档里,页面样式照样管得到。
        //
        // 尺寸先给个占位:有 shadow root 的宿主**不会渲染没被 slot 分走的
        // 子节点**,不先把节点 slot 进去,下面量尺寸量到的全是 0(标题就是
        // 被这个坑了一版)。占位给大一点只是不想让引擎有机会按窄视口掐它 ——
        // 标题是 nowrap 的 inline-block,量出来本来就跟视口多宽无关。
        for (const n of this.nodes) {
            const fo = el('foreignObject', {
                class: 'g-node',
                width: FO_PROBE_W,
                height: FO_PROBE_H,
            }, svg);
            const s = document.createElement('slot');
            s.name = n.slot;
            fo.appendChild(s);
            n.fo = fo;
        }

        // 节点可能早于自己的子节点被 upgrade(见 GraphNode.build),这里文档
        // 已就绪,补一刀。build() 末尾自己会 relayout —— 只有**已经**建过的
        // 节点要补:它当初量尺寸时还没被 slot 进去,量到的全是 0。
        for (const n of this.nodes) {
            if (n.built) n.relayout();
            else n.build();
        }

        // 读连接。端点指向哪根引脚只有把整张图的节点都收齐了才知道,所以
        // 「节点不存在」「序号越界」在这儿报 —— 构建期报不了,每条连接写出来
        // 的时候图还没写完。报完跳过这一条,少一条线总比整张图炸掉好。
        for (const c of this.querySelectorAll('connection')) {
            const kind = c.getAttribute('data-kind');
            const fi = c.getAttribute('data-from-index');
            const ti = c.getAttribute('data-to-index');
            const from = this.byId.get(c.getAttribute('data-from-node'));
            const to = this.byId.get(c.getAttribute('data-to-node'));
            if (!from || !to) {
                const missing = from ? c.getAttribute('data-to-node') : c.getAttribute('data-from-node');
                console.error(`graph: 连接引用了不存在的节点:${missing}`, c);
                continue;
            }
            const fpId = `${kind}-out-${fi}`;
            const tpId = `${kind}-in-${ti}`;
            if (!from.hasPin(fpId) || !to.hasPin(tpId)) {
                const side = from.hasPin(fpId) ? to : from;
                const n = (side === from ? fi : ti);
                console.error(
                    `graph: 节点 ${side.id} 的第 ${n} 根 ${kind}-${side === from ? 'out' : 'in'} 引脚不存在`,
                    c);
                continue;
            }
            const e = {
                kind, from, to, fpId, tpId,
                type: this.byType(to, kind) || this.byType(from, kind),
            };
            this.edges.push(e);
            this.adj.get(from).push(e);
            this.adj.get(to).push(e);
        }

        this.#layout();

        for (const e of this.edges) {
            e.hit = el('path', { class: 'g-edge-hit' }, edgeLayer);
            // 类型只在悬停这条线时浮出来。
            if (e.type) el('title', {}, e.hit).textContent = e.type;
            e.el = el('path', { class: `g-edge ${e.kind}` }, edgeLayer);
            this.#redrawEdge(e);
        }

        this.querySelector('.graph-fallback')?.remove();

        // 重置按钮挂在 .graph-root 上,不进 .g-layer —— 不跟着缩放平移走,
        // 一直待在视口右上角。每次重算都重新挂一个(前面 replaceChildren 清了)。
        // 用 createElement 而不是 el():el() 一律建 SVG 名字空间的元素,HTML
        // 元素那样建出来在页面里没有盒子(rect 全 0)。
        const reset = document.createElement('button');
        reset.className = 'g-reset';
        reset.type = 'button';
        reset.textContent = '⟲';
        root.appendChild(reset);
        reset.title = '重新布局并复位视图';
        reset.addEventListener('click', () => this.#rebuild());
    }

    /** 节点上某个 kind 的引脚类型(取第一根)—— 只用来填悬停提示。 */
    byType(node, kind) {
        const p = node.pins.find(q => q.kind === kind && q.type);
        return p ? p.type : '';
    }

    // ---- 分层布局 ----
    #layout() {
        const n = this.nodes.length;
        if (!n) return;
        const idx = new Map(this.nodes.map((v, i) => [v.id, i]));

        // 分层:反复松弛「rank[to] >= rank[from] + 1」,最多 n 轮。这是
        // Bellman-Ford 的形状,能处理环(环里的边顶多让层数白涨一点,不会
        // 死循环)。遍历一律按文档顺序。
        const rank = new Array(n).fill(0);
        const eIdx = this.edges.map(e => [idx.get(e.from.id), idx.get(e.to.id)]);
        for (let round = 0; round < n; round++) {
            let changed = false;
            for (const [a, b] of eIdx) {
                if (rank[b] < rank[a] + 1) {
                    rank[b] = rank[a] + 1;
                    changed = true;
                }
            }
            if (!changed) break;
        }

        // 分列,列内保持文档顺序
        const cols = [];
        for (let i = 0; i < n; i++) {
            const r = rank[i];
            (cols[r] || (cols[r] = [])).push(this.nodes[i]);
        }

        // 两轮重心排序:每列按「前驱的平均行号」排,让连线少交叉。平手一律
        // 保留文档顺序(`.sort` 在 V8 上是稳定的)。
        const rowOf = new Map();
        const layOut = c => c.forEach((v, i) => rowOf.set(v, i));
        cols.forEach(layOut);
        for (let pass = 0; pass < 2; pass++) {
            for (let ci = 1; ci < cols.length; ci++) {
                const bary = new Map();
                for (const v of cols[ci]) {
                    const ps = this.edges.filter(e => e.to === v).map(e => rowOf.get(e.from));
                    bary.set(v, ps.length ? ps.reduce((a, b) => a + b, 0) / ps.length : rowOf.get(v));
                }
                cols[ci].sort((a, b) => bary.get(a) - bary.get(b));
                layOut(cols[ci]);
            }
        }

        // 落坐标
        let x = PAD;
        for (const col of cols) {
            const cw = Math.max(...col.map(v => v.w));
            let y = PAD;
            for (const v of col) {
                v.setPosition(Math.round(x + (cw - v.w) / 2), Math.round(y));
                y += v.h + ROW_GAP;
            }
            x += cw + COL_GAP;
        }
    }

    #redrawEdge(e) {
        const fp = e.from.worldAnchor(e.fpId);
        const tp = e.to.worldAnchor(e.tpId);
        if (!fp || !tp) return;
        const d = edgePath(fp.x, fp.y, tp.x, tp.y);
        e.el.setAttribute('d', d);
        e.hit.setAttribute('d', d);
    }

    // ---- 拖动 / 缩放 ----
    #bindDrag(root) {
        // 视图状态挂在实例上,重置按钮够得着;监听器只在 #build 里挂一次,
        // 重算时复用同一个 .graph-root。
        const view = this.view = { scale: 1, tx: 0, ty: 0 };
        // 一个 CSS 变量广播给两边:节点的 div 和连线层各自 `var(--g-tf)`。
        // 变量是活的,改一次全都动,不用挨个重设。
        const applyPan = () => {
            this.style.setProperty('--g-tf', `translate(${view.tx}px, ${view.ty}px) scale(${view.scale})`);
        };
        view.reset = () => {
            view.scale = 1;
            view.tx = 0;
            view.ty = 0;
            applyPan();
        };
        applyPan();

        // 屏幕坐标 → 世界坐标。节点只经过上面这一个变换,减掉平移再除缩放
        // 就完了。
        const toLocal = (cx, cy) => {
            const b = root.getBoundingClientRect();
            return {
                x: (cx - b.left - view.tx) / view.scale,
                y: (cy - b.top - view.ty) / view.scale,
            };
        };

        let mode = null;
        let dragging = null;
        let start = null;
        // 光标要挂两处:.graph-root 在 shadow 里(拖动时指针被捕获在那儿,只有
        // 它自己的光标说了算),this 在 light DOM(节点平时没被捕获时走那条)。
        const setMode = m => {
            mode = m;
            root.classList.toggle('panning', m === 'pan');
            root.classList.toggle('dragging', m === 'node');
            this.classList.toggle('panning', m === 'pan');
            this.classList.toggle('dragging', m === 'node');
        };

        root.addEventListener('pointerdown', ev => {
            // 交互元素(重置按钮、节点标题里的链接)不参与拖动:一旦
            // setPointerCapture,pointerup 就归 .graph-root,click 也派发到它
            // 身上 —— 按钮和链接就再也收不到 click 了。
            if (ev.target.closest?.('button, a, input, select, textarea')) return;
            const nodeEl = ev.target.closest?.('ui-graph-node');
            // setPointerCapture 收住后续事件,否则指针拖出 svg 就断了。
            root.setPointerCapture(ev.pointerId);
            // 两份起点:世界坐标(拖节点用)和屏幕坐标(平移用)。平移的
            // 增量是屏幕像素,不能再过一遍 /scale —— 那等于把位移又缩放
            // 了一次,缩放之后越拖越飞。
            const p = toLocal(ev.clientX, ev.clientY);
            start = { sx: p.x, sy: p.y, cx: ev.clientX, cy: ev.clientY, ox: 0, oy: 0 };
            if (ev.button === 2 || ev.button === 1) {
                // 右键 / 中键平移整张图。
                setMode('pan');
            } else if (ev.button === 0) {
                // 左键拖节点 —— 鼠标和触摸一样,按在哪个节点上就拖哪个。
                const n = nodeEl ? this.byEl.get(nodeEl) : null;
                if (n) {
                    setMode('node');
                    dragging = n;
                    start.ox = n.x;
                    start.oy = n.y;
                } else if (ev.pointerType === 'touch') {
                    // 触摸没有右键,图总得有个搬的办法:单指按在空白处就平移。
                    // 鼠标的空白处左键仍然什么都不做。
                    setMode('pan');
                }
            }
            if (!mode) return;
            ev.preventDefault();
        });

        root.addEventListener('pointermove', ev => {
            if (!mode || !start) return;
            if (mode === 'pan') {
                // 平移吃屏幕像素:图跟手,跟缩放无关。
                view.tx += ev.clientX - start.cx;
                view.ty += ev.clientY - start.cy;
                start.cx = ev.clientX;
                start.cy = ev.clientY;
                applyPan();
                return;
            }
            // 拖节点吃世界坐标:图缩放多少,指针走过的世界距离就是多少。
            const p = toLocal(ev.clientX, ev.clientY);
            dragging.setPosition(
                Math.round(start.ox + (p.x - start.sx)),
                Math.round(start.oy + (p.y - start.sy)),
            );
            // 引脚的相对坐标没变,只有盒子动了 —— 只重画挂在它身上的那些线。
            for (const e of this.adj.get(dragging)) this.#redrawEdge(e);
        });

        const end = ev => {
            if (root.hasPointerCapture?.(ev.pointerId)) root.releasePointerCapture(ev.pointerId);
            dragging = null;
            start = null;
            setMode(null);
        };
        root.addEventListener('pointerup', end);
        root.addEventListener('pointercancel', end);
        root.addEventListener('contextmenu', ev => ev.preventDefault());

        root.addEventListener('wheel', ev => {
            ev.preventDefault();
            const f = ev.deltaY < 0 ? 1.1 : 1 / 1.1;
            const next = Math.max(0.3, Math.min(3, view.scale * f));
            if (next === view.scale) return;
            // 缩放锚在指针下的那个点:先记下它的世界坐标,缩放之后把平移
            // 调回去,让同一个世界点仍落在指针下面。
            const b = root.getBoundingClientRect();
            const wx = (ev.clientX - b.left - view.tx) / view.scale;
            const wy = (ev.clientY - b.top - view.ty) / view.scale;
            view.scale = next;
            view.tx = ev.clientX - b.left - wx * next;
            view.ty = ev.clientY - b.top - wy * next;
            applyPan();
        }, { passive: false });

        // 悬停:节点自己发事件(它在自己的 shadow 里,父级够不着)。
        root.addEventListener('graph-hover', ev => this.#hot(ev.detail.on ? ev.detail.id : null));
    }

    /** 悬停:高亮这个节点和它的进出线,其余淡出。 */
    #hot(id) {
        const hot = id ? this.byId.get(id) : null;
        // 邻接表已经建好,这里不必再拿边表全扫一遍(O(N·E))。
        const near = new Set(hot ? this.adj.get(hot).flatMap(e => [e.from, e.to]) : []);
        for (const n of this.nodes) {
            n.markHot(n === hot);
            n.dim(!!hot && !near.has(n));
        }
        for (const e of this.edges) {
            const on = !!hot && (e.from === hot || e.to === hot);
            e.el.classList.toggle('hot', on);
            e.el.classList.toggle('dim', !!hot && !on);
        }
    }
});