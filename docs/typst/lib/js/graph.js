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
        // 和 ui-graph 同一个坑:脚本在 <head>,自定义元素在解析到开标签时
        // 就触发 connectedCallback,此刻子节点还没进 light DOM。等文档
        // 就绪再建。ui-graph 也会主动调 build(),所以幂等。
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', () => this.build(), { once: true });
        } else {
            this.build();
        }
    }

    /** 建 shadow:盒子 + 引脚形状。幂等 —— 早于子节点解析时是空跑,等
     * ui-graph 在自己那次 build 里再调一次。 */
    build() {
        if (this.built) return;
        // 解析中:子节点(pin)还没进来,现在建等于建个空盒子。等 ui-graph
        // 在文档就绪后回头调这一下。
        if (document.readyState === 'loading') return;
        this.built = true;

        // 引脚就在自己的 light DOM 里。没有 id —— 同 kind + 同 dir 算一组,
        // 组内第几号在这儿数出来。
        const pins = [...this.querySelectorAll('pin')].map(p => ({
            dir: p.getAttribute('data-dir'),
            kind: p.getAttribute('data-kind'),
            // 名字写在 pin 的内容里,渲染时显示在引脚旁;类型只在悬停时
            // 浮出来,平时不占地方。
            label: (p.textContent || '').trim(),
            type: p.getAttribute('data-type') || '',
        }));
        const counts = new Map();
        for (const p of pins) {
            const g = `${p.kind}-${p.dir}`;
            const i = counts.get(g) || 0;
            // 别省 0:连接里两端都写全序号,这里也一律带,两边的规则要一样。
            p.id = `${g}-${i}`;
            counts.set(g, i + 1);
        }
        // 槽位按边分:控制流排在数据流上面(viewer 也是这个次序,看着顺)。
        for (const side of ['in', 'out']) {
            pins
                .filter(p => p.dir === side)
                .sort((a, b) => (a.kind === 'ctrl' ? 0 : 1) - (b.kind === 'ctrl' ? 0 : 1))
                .forEach((p, i) => { p.slot = i; });
        }
        this.pins = pins;
        this.points = new Map();

        const shadow = this.attachShadow({ mode: 'open' });
        const style = document.createElement('style');
        // shadow 内部拿不到页面的 CSS,所以这份样式必须写在这里。
        style.textContent = `
            /* 元素左右各让出 BLEED 的 padding,.g-box 缩在圈里面,引脚形状才
             * 伸得出去。宽窄由 .g-box 的 min-width 定(见 relayout)。 */
            :host { display: block; position: relative; box-sizing: content-box;
                padding: 0 ${BLEED}px; }
            /* width: max-content 让它自己收到标题那么宽;min-width 由 JS 按
             * 引脚排布算好后写回来。标题就是 slot 投影进来的那个文本节点,
             * ::slotted 只管元素,管不到文本 —— 所以字形得写在盒子上。 */
            .g-box { position: relative; width: max-content; min-width: ${NODE_MIN_W}px;
                height: 100%; box-sizing: border-box; background: #fff;
                border: 1.5px solid #c9cbd4; border-radius: 7px; overflow: hidden;
                font-family: inherit; font-size: ${TITLE_PX}px; font-weight: 600;
                line-height: 21px; color: #19181f; white-space: nowrap;
                /* 标题是投影进来的裸文本节点,没法单独给它设 margin,靠盒子
                 * 的上内边距把它压到标题条中间去。 */
                padding: ${NODE_HEAD_PAD}px ${NODE_PAD_X}px 0; }
            :host(:hover) .g-box { border-color: #11566c; }
            :host(.hot) .g-box { border-color: #11566c; border-width: 2.5px; }
            :host(.dim) { opacity: .28; }
            /* 引脚也是 slot 投影进来的,但它们只是数据,不当内容显示。 */
            ::slotted(pin) { display: none; }
            .g-shapes { position: absolute; left: 0; top: 0; }
            .g-label { fill: #7a7c86; font-size: ${PIN_PX}px; dominant-baseline: middle; }
            .g-ctrl { fill: #11566c; }
            .g-data { fill: #fff; stroke: #11566c; stroke-width: 1.5; }
        `;
        shadow.appendChild(style);

        // 盒子里只有一个无名 <slot>:light DOM 里的标题文本节点连同一堆
        // <pin> 全投影进来,引脚由 ::slotted(pin) 藏掉,剩下的就是标题。
        this.box = document.createElement('div');
        this.box.className = 'g-box';
        this.box.appendChild(document.createElement('slot'));
        shadow.appendChild(this.box);

        this.svg = el('svg', { class: 'g-shapes' }, shadow);

        for (const p of pins) {
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
                p.textEl = el('text', {
                    class: 'g-label',
                    y: 0,
                    'text-anchor': p.dir === 'in' ? 'start' : 'end',
                }, this.svg);
                p.textEl.textContent = p.label;
            }
        }

        this.addEventListener('pointerenter', () =>
            this.dispatchEvent(new CustomEvent('graph-hover', { detail: { id: this.id, on: true } })));
        this.addEventListener('pointerleave', () =>
            this.dispatchEvent(new CustomEvent('graph-hover', { detail: { id: this.id, on: false } })));

        // 自己量一遍。在图里 ui-graph 也会调,重复调没副作用;不在图里
        // (单独摆一个节点)就靠这一下。
        this.relayout();
    }

    /**
     * 量出盒子尺寸、算出每根引脚相对盒子左上角的坐标,存进 this.points。
     *
     * 标题就是 slot 投影进来的那段文本,宽度直接量盒子的内容宽 —— 不是按字宽
     * 估算,长名字不会把节点挤窄。引脚标签是 SVG 文本,用 `getComputedTextLength`。
     * 前提是节点已在文档里且可见(display:none 时量出来是 0)。
     */
    relayout() {
        // 形状以盒子边界为中心:圆心/三角心正落在边界上。往盒子**里面**伸的
        // 那半截 + 到标签的空 + 标签,就是从边界起算一根引脚占的宽 —— 排版只
        // 关心朝里这半截,朝外那半截由元素让出的 BLEED 兜着。
        const half = p => (p.kind === 'ctrl' ? CTRL_R : PIN_R + 1);
        const extent = p => half(p) + LABEL_GAP +
            (p.textEl ? Math.ceil(p.textEl.getComputedTextLength()) : 0);

        this.rows = Math.max(1, ...this.pins.map(p => p.slot + 1));
        // 左右各按行量:同一行(i 号槽位)的入口和出口在同一个 y 上,会撞。
        const rowIn = new Array(this.rows).fill(0);
        const rowOut = new Array(this.rows).fill(0);
        for (const p of this.pins) {
            const row = p.dir === 'in' ? rowIn : rowOut;
            row[p.slot] = Math.max(row[p.slot], extent(p));
        }

        // .g-box 是 border-box,量到的 rect 就是盒子外宽。this.w 一律指外宽,
        // 引脚坐标直接落在 BLEED / BLEED + w 上。
        let w = Math.max(NODE_MIN_W, Math.ceil(this.box.getBoundingClientRect().width));
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
            p.y = NODE_HEAD_H + PIN_TOP + p.slot * PIN_GAP + PIN_GAP / 2;
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
            if (p.textEl) {
                p.textEl.setAttribute('x',
                    p.dir === 'in' ? p.x + hw + LABEL_GAP : p.x - hw - LABEL_GAP);
                p.textEl.setAttribute('y', p.y);
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
        // 脚本在 <head>,自定义元素先于它的子节点被解析 —— connectedCallback
        // 触发时子节点还没进 light DOM,直接读会读到空的。推迟到文档就绪后
        // 再读(只推迟一次;`done` 挡住后续的重复调用)。
        if (this.done) return;
        this.done = true;
        const run = () => this.#build();
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', run, { once: true });
        } else {
            run();
        }
    }

    #build() {
        this.nodes = [];
        this.edges = [];
        this.byId = new Map();

        // 节点就是文档里写的那些 <ui-graph-node> —— id 直接用 HTML 的 id。
        // id 可以不给(单独摆一个看看时不必费劲起名),那种节点不进 byId,
        // 连接自然也就引用不到它。
        for (const n of this.querySelectorAll('ui-graph-node')) {
            if (n.id) {
                if (this.byId.has(n.id)) {
                    console.error(`graph: 节点 id 重复:${n.id}`, n);
                }
                this.byId.set(n.id, n);
            }
            // <slot name> 靠元素的 slot 属性认领。没 id 的节点(单独摆一个看看
            // 形状时)按文档顺序补个名字。改属性不改结构,元素还在原地。
            n.slot = n.id || `n${this.nodes.length}`;
            this.nodes.push(n);
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
            /* 平移整张图:右键 / 中键拖。手型而不是十字 —— 一路手型,
             * 按下去和平时长得一样。空白处左键不动,光标就还是默认的。 */
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
        `;
        shadow.appendChild(style);
        shadow.appendChild(root);

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
            this.edges.push({
                kind, from, to, fpId, tpId,
                type: this.byType(to, kind) || this.byType(from, kind),
            });
        }

        this.#layout();

        for (const e of this.edges) {
            e.hit = el('path', { class: 'g-edge-hit' }, edgeLayer);
            // 类型只在悬停这条线时浮出来。
            if (e.type) el('title', {}, e.hit).textContent = e.type;
            e.el = el('path', { class: `g-edge ${e.kind}` }, edgeLayer);
            this.#redrawEdge(e);
        }

        this.#bindDrag(root);
        this.querySelector('.graph-fallback')?.remove();
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
        let scale = 1, tx = 0, ty = 0;
        // 一个 CSS 变量广播给两边:节点的 div 和连线层各自 `var(--g-tf)`。
        // 变量是活的,改一次全都动,不用挨个重设。
        const applyPan = () => {
            this.style.setProperty('--g-tf', `translate(${tx}px, ${ty}px) scale(${scale})`);
        };
        applyPan();

        // 屏幕坐标 → 世界坐标。节点只经过上面这一个变换,减掉平移再除缩放
        // 就完了。
        const toLocal = (cx, cy) => {
            const b = root.getBoundingClientRect();
            return { x: (cx - b.left - tx) / scale, y: (cy - b.top - ty) / scale };
        };

        let mode = null;
        let dragging = null;
        let start = null;

        root.addEventListener('pointerdown', ev => {
            const nodeEl = ev.target.closest?.('ui-graph-node');
            // setPointerCapture 收住后续事件,否则指针拖出 svg 就断了。
            root.setPointerCapture(ev.pointerId);
            // 两份起点:世界坐标(拖节点用)和屏幕坐标(平移用)。平移的
            // 增量是屏幕像素,不能再过一遍 /scale —— 那等于把位移又缩放
            // 了一次,缩放之后越拖越飞。
            const p = toLocal(ev.clientX, ev.clientY);
            start = { sx: p.x, sy: p.y, cx: ev.clientX, cy: ev.clientY, ox: 0, oy: 0 };
            if (ev.button === 2 || ev.button === 1) {
                // 右键 / 中键平移整张图。左键留给节点 —— 空白处左键什么
                // 都不做,免得拖个空白就把整张图带走。
                mode = 'pan';
            } else if (ev.button === 0 && nodeEl) {
                mode = 'node';
                dragging = this.byId.get(nodeEl.id);
                start.ox = dragging.x;
                start.oy = dragging.y;
            } else {
                mode = null;
            }
            root.classList.toggle('panning', mode === 'pan');
            root.classList.toggle('dragging', mode === 'node');
            this.classList.toggle('panning', mode === 'pan');
            this.classList.toggle('dragging', mode === 'node');
            ev.preventDefault();
        });

        root.addEventListener('pointermove', ev => {
            if (!mode || !start) return;
            if (mode === 'pan') {
                // 平移吃屏幕像素:图跟手,跟缩放无关。
                tx += ev.clientX - start.cx;
                ty += ev.clientY - start.cy;
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
            // 引脚的相对坐标没变,只有盒子动了 —— 重画挂在它身上的线。
            for (const e of this.edges) {
                if (e.from === dragging || e.to === dragging) this.#redrawEdge(e);
            }
        });

        const end = ev => {
            if (root.hasPointerCapture?.(ev.pointerId)) root.releasePointerCapture(ev.pointerId);
            mode = null; dragging = null; start = null;
            root.classList.remove('panning');
            root.classList.remove('dragging');
            this.classList.remove('panning');
            this.classList.remove('dragging');
        };
        root.addEventListener('pointerup', end);
        root.addEventListener('pointercancel', end);
        root.addEventListener('contextmenu', ev => ev.preventDefault());

        root.addEventListener('wheel', ev => {
            ev.preventDefault();
            const f = ev.deltaY < 0 ? 1.1 : 1 / 1.1;
            const next = Math.max(0.3, Math.min(3, scale * f));
            if (next === scale) return;
            // 缩放锚在指针下的那个点:先记下它的世界坐标,缩放之后把平移
            // 调回去,让同一个世界点仍落在指针下面。
            const b = root.getBoundingClientRect();
            const wx = (ev.clientX - b.left - tx) / scale;
            const wy = (ev.clientY - b.top - ty) / scale;
            scale = next;
            tx = ev.clientX - b.left - wx * scale;
            ty = ev.clientY - b.top - wy * scale;
            applyPan();
        }, { passive: false });

        // 悬停:节点自己发事件(它在自己的 shadow 里,父级够不着)。
        root.addEventListener('graph-hover', ev => this.#hot(ev.detail.on ? ev.detail.id : null));
        this.root = root;
    }

    /** 悬停:高亮这个节点和它的进出线,其余淡出。 */
    #hot(id) {
        const hot = id ? this.byId.get(id) : null;
        for (const n of this.nodes) {
            n.markHot(n === hot);
            const near = hot && (n === hot ||
                this.edges.some(e => e.from === hot && e.to === n) ||
                this.edges.some(e => e.to === hot && e.from === n));
            n.dim(!!hot && !near);
        }
        for (const e of this.edges) {
            const on = !!hot && (e.from === hot || e.to === hot);
            e.el.classList.toggle('hot', on);
            e.el.classList.toggle('dim', !!hot && !on);
        }
    }
});