# 摆弄页 `.gia` 预览设计

日期：2026-10-19
状态：待实现

## 目标

把摆弄页「预览」选项卡从一行产物说明升级成真正的节点图浏览器：

- 后端在同一个 HTTP 服务上把 `core/proto/asset.proto` 原样发出去；
- 前端用 protobuf.js 解析收到的 `.gia` 字节，**不上传、不回服务端解码**；
- 预览窗用 class 驱动状态（空 / 解析中 / 就绪 / 失败），非就绪时内部结构整体隐藏；
- 外层一条只有图标的左边栏：**图** / **结构体**（结构体先占位「待完善」）；
- 图视图里再有一条左边栏列出所有图，选中后在右侧渲染该图；
- 每张图复用文档站已有的 `<ui-graph>` 自定义元素，保留拖动、平移、缩放、悬停高亮；
- 收到产物就开始异步解析与构建，全部完成后才把预览窗切到就绪态。

预览只是一个**只读视图**：不编辑、不回写、不影响编译与下载。

## 非目标

- 不解析 `NodeGraphData.embedded`。后端目前把它们写成 `data: None`（`core/src/node/mod.rs:753-758`），没有可渲染的子图。
- 不做「结构体」页面的实际内容。本版只有「待完善」占位。
- 不复活节点在游戏编辑器里的原始坐标。`ui-graph` 自己按连接做确定性分层布局，`x_pos` / `y_pos` 不参与。
- 不给系统节点起名字。`.gia` 的 `NodeInstance` 里没有显示名，只有 `shell_ref`（`core/proto/asset.proto:752-797`）；本版标题是 `#序号 · 类型`。
- 不做预览与上次产物的 diff，不做导出图片。
- 不在服务端解码 `.gia`。`.proto` 只是给浏览器解析用的 schema。

## 现状

- 产物以裸字节经 WebSocket 二进制帧到浏览器，存在 `artifactBytes`（`docs/typst/play/script.js:1346`、`:1469-1474`），下载走 `Blob`。
- 预览面板只有一句占位：`.play-preview:empty::before { content: "待编译" }`（`docs/typst/play/style.css:702-713`），收到产物后 JS 把一句「产物 … · 大小」写进 `textContent`（`script.js:1574-1582`）。
- 面板切换只动 `hidden` 属性（`script.js:1583-1591`），`.play-panel[hidden] { display: none }`（`style.css:630-632`）。
- `.gia` 的封装格式在 `core/src/asset.rs:109-136`：20 字节大端头（`[总大小-4, 1, 0x0326, 3, proto 长度]`）+ proto 负载 + 4 字节尾 `0x0679`。
- 文档站的 `ui-graph` / `ui-graph-node` 已在 `docs/typst/lib/js/graph.js` 实现，注册入口是 `docs/typst/lib/graph.typ:41-44`；摆弄页目前**没有**引入它。
- `asset.proto` 的 package 是 `rust2genshin`（`core/proto/asset.proto:4`），根消息 `AssetBundleData` 在 `:13`。

## 架构

```text
浏览器                                   后端（playground）
  │  GET /proto/asset.proto                    │
  │ ─────────────────────────────────────────► │  tokio::fs::read(repo_dir/core/proto/asset.proto)
  │ ◄───────────────────────────────────────── │  text/plain
  │
  │  WebSocket 二进制帧（.gia 原始字节）
  │ ◄───────────────────────────────────────── │
  │
  ├─ gia.js：剥 20 字节头 / 4 字节尾
  ├─ protobuf.js：AssetBundleData.decode + toObject
  ├─ 为每张图构建 detach 的 <ui-graph> 模板
  └─ 预览面板可见时，挂选中那张图
```

职责边界：

| 文件 | 职责 |
|---|---|
| `playground/src/server/http.rs` | 新增 `/proto/asset.proto` 路由；只读仓库里的文件，不做缓存与转换 |
| `docs/typst/play/gia.js` | 纯数据层：剥壳、加载/缓存 schema、解码、收集图、构建 `ui-graph` 元素 |
| `docs/typst/play/script.js` | 状态机、DOM 挂载、列表/侧栏交互、与编译生命周期对接 |
| `docs/typst/play/index.typ` | 预览窗静态骨架；注册 `graph.js` / `graph.css` |
| `docs/typst/play/style.css` | 状态 class、双左边栏、画布与占位样式 |

`ui-graph` 本身不改。它已经是文档站里被验证过的组件，摆弄页只是复用。

## 后端：`/proto/asset.proto`

`playground/src/server/http.rs` 里仿照 `/sample`（`:68-70`、`:158-177`）加：

```rust
/// schema 是给浏览器解析 `.gia` 用的；跟着仓库走，不 embed。
pub fn proto_path(cfg: &Config) -> PathBuf {
    cfg.repo_dir.join("core/proto/asset.proto")
}

async fn asset_proto(State(st): State<Arc<AppState>>) -> Response {
    let Ok(bytes) = tokio::fs::read(proto_path(&st.cfg)).await else {
        return (StatusCode::NOT_FOUND, "asset.proto 不在仓库里").into_response();
    };
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], bytes).into_response()
}
```

路由挂在 CORS 层之前（`http.rs:113-120`）：

```rust
.route("/proto/asset.proto", get(asset_proto))
```

要点：

- **每次请求都读文件**，理由和 `/sample` 一样：部署的就是仓库工作区，schema 不应该在进程启动时被冻结；文件很小，没有性能问题。
- 不设长缓存头。schema 与代码同源，缓存住反而会在后端升级后让浏览器拿旧 schema。
- CORS 沿用现有层：部署时 `PLAYGROUND_ALLOWED_ORIGINS=*`，GitHub Pages 上的页面能直接读。
- **版本错位可容忍**：如果镜像烘焙后仓库又前进了，`.proto` 可能比产物里的数据新/旧。protobuf 对未知字段与缺失字段都按默认值处理，预览最坏是少几个字段；只有 schema 语法本身坏掉才会进 `is-error`。

## 前端数据层：`play/gia.js`

新增 `docs/typst/play/gia.js`，ESM。`script.js` 顶部 `import { … } from "./gia.js"`。

### 剥壳

```js
export function unwrapGia(bytes) {
    if (bytes.length < 24) throw new Error("文件太短，不是 .gia");
    const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const total = v.getUint32(0);      // 总大小 - 4
    const schema = v.getUint32(4);     // 1
    const magic = v.getUint32(8);      // 0x0326
    const type = v.getUint32(12);      // 3 = AssetBundle
    const len = v.getUint32(16);
    const tail = v.getUint32(bytes.length - 4);
    if (total !== bytes.length - 4 || schema !== 1 || magic !== 0x0326 ||
        type !== 3 || len !== bytes.length - 24 || tail !== 0x0679) {
        throw new Error("文件头校验失败，不是 .gia");
    }
    return bytes.subarray(20, bytes.length - 4);
}
```

常量与 `core/src/asset.rs:109-136` 一一对应。任何一项不对都直接报错——猜着解析只会得到更难懂的错误。

### 加载与缓存 schema

```js
let schemaCache = null; // { origin, type }

export async function loadSchema(origin) {
    if (schemaCache?.origin === origin) return schemaCache.type;
    const res = await fetch(`${origin}/proto/asset.proto`);
    if (!res.ok) throw new Error(`取 schema 失败（HTTP ${res.status}）`);
    const text = await res.text();
    const protobuf = await import("https://esm.sh/protobufjs@7.4.0");
    const { root } = protobuf.parse(text);           // package 自带 rust2genshin
    const type = root.lookupType("rust2genshin.AssetBundleData");
    schemaCache = { origin, type };
    return type;
}
```

- protobuf.js **按需动态 import**：不编译就不下载这 ~100 KB。
- 按 origin 缓存：`BACKENDS` 可能切到另一台后端，schema 得跟着换。
- 已用 7.4.0 对当前 `asset.proto` 做过可行性验证（嵌套消息、`oneof`、proto3 `optional`、packed 都能解析）。

### 解码与收集图

```js
export function decodeBundle(type, bytes) {
    const payload = unwrapGia(bytes);
    return type.toObject(type.decode(payload), {
        longs: String,   // guid / runtime_id 是 int64
        enums: String,   // shell_ref.kind 等直接拿到名字
        defaults: false,
    });
}

export function collectGraphs(bundle) {
    const out = [];
    for (const asset of [...(bundle.assets ?? []), ...(bundle.dependencies ?? [])]) {
        const graph = asset.graphData?.inner?.graph;
        if (!graph) continue;
        out.push({
            name: graph.displayName || asset.name || `图 ${out.length + 1}`,
            nodeCount: (graph.node ?? []).length,
            graph,
        });
    }
    return out;
}
```

顺序就是 `assets`（主图在前）再加 `dependencies`，和产物里的声明顺序一致。

### 构建 `ui-graph`

`buildGraphElement(graph)` 返回一个 **从未插入过文档** 的 `<ui-graph>`。因为未连接，`ui-graph` 与 `ui-graph-node` 的 `connectedCallback` 不会跑，也就不会在 `display:none` 的祖先下量出 0 尺寸。

节点：

```js
const el = document.createElement("ui-graph");
const nodeByIndex = new Map();
for (const n of graph.node ?? []) {
    const node = document.createElement("ui-graph-node");
    node.id = `n${n.index}`;
    node.title = nodeDetail(n);                 // 悬停可见
    node.textContent = nodeTitle(n);            // "#12 · 系统 180"
    nodeByIndex.set(n.index, { index: n.index, el: node, table: new Map() });

    for (const p of pinPlan(n.pins ?? [])) {    // 见下：排序 + 编号
        const pin = document.createElement("pin");
        pin.dataset.dir = p.dir;
        pin.dataset.kind = p.kind;
        pin.dataset.type = p.typeId == null ? "" : String(p.typeId);
        node.appendChild(pin);
    }
    el.appendChild(node);
}
```

**产物里的引脚和连接不是对称的，排引脚前必须先理解三件事**（依据 `core/src/node/mod.rs:612-713`）：

1. `IN_CONTROL` **根本不进 proto**：后端只发 `controls_out`、`values_out`、`values_in`。控制边的输入端只出现在 `NodeConnection.target_pin_shell` 里，节点上没有对应引脚。
2. **没有 selector 的槽不发引脚**：字面量、取变量这类节点的输出槽就是 `selectors_out[i] == None`，别的节点照样连到它的 `OUT_VALUE`。
3. **数据边在两端各记一次**：同一条线既写进输出引脚的 `connection`，也写进输入引脚的 `connection`（`handle_value` 里 `values_out` 与 `values_in.link` 各调一次）。

因此引脚的来源是「声明过的 ∪ 连接引用过的」，缺的补出来；画边前按两端去重。

`PinSignature.Kind` 到 `ui-graph` 的映射（`core/proto/asset.proto:800-824`）：

| Kind | 值 | `data-dir` | `data-kind` |
|---|---:|---|---|
| `IN_CONTROL` | 1 | `in` | `ctrl` |
| `OUT_CONTROL` | 2 | `out` | `ctrl` |
| `IN_VALUE` | 3 | `in` | `data` |
| `OUT_VALUE` | 4 | `out` | `data` |
| 其余（META / STRUCT_*） | — | 跳过 | — |

引脚补全与排序：

1. 第一遍只扫边，把每条边的输出端 `(node, out-kind, shell index)` 与输入端 `(node, in-kind, shell index)` 记进 `referenced`；
2. 每个节点按四组（`in/ctrl`、`out/ctrl`、`in/data`、`out/data`）各排各的：声明过的引脚取 `shell_sig.index`，没声明但被引用的补一个空引脚；
3. 组内按 index 升序后从 0 起重新编号，得到 `ordinal`，写进登记表 `` `${node}:${dir}-${kind}` `` → (index → ordinal)。`dir` 必须进键 —— 输入和输出各有一根 `data-0`。

**引脚顺序必须按 `shell_sig.index` 排**：`graph.js:186-196` 按 DOM 出现顺序编号 `${kind}-${dir}-${i}`，而连接引用的是「同类型引脚里的第几个」。排完 DOM 序号才与协议序号一致。

`PinData` 没有显示名（`:834-857`），所以引脚标签留空：形状在，悬停时浮出类型 ID。这是本版接受的粗糙点。

连接：

```js
// 统一成「输出端 → 输入端」：
//   载体是 OUT_* 时，目标就是输入端；
//   载体是 IN_VALUE 时（数据边的另一份），target_pin_shell 才是源。
// 两端各记一次，所以按「源节点:序号 → 目标节点:序号:类型」去重。
const seen = new Set();
forEachEdge(nodes, (edge) => {
    const from = pinOrdinal(records, edge.fromNode, "out", edge.kind, edge.fromSig);
    const to = pinOrdinal(records, edge.toNode, "in", edge.kind, edge.toSig);
    if (from == null || to == null) return;
    const key = `${edge.fromNode}:${from}->${edge.toNode}:${to}:${edge.kind}`;
    if (seen.has(key)) return;
    seen.add(key);
    const conn = document.createElement("connection");
    conn.dataset.kind = edge.kind;              // ctrl / data
    conn.dataset.fromNode = `n${edge.fromNode}`;
    conn.dataset.fromIndex = String(from);
    conn.dataset.toNode = `n${edge.toNode}`;
    conn.dataset.toIndex = String(to);
    el.appendChild(conn);
});
```

`pinOrdinal` 在登记表里按 `` `${dir}-${kind}-${shellSig.index ?? 0}` `` 查；查不到（节点不存在、类型对不上）就跳过这一条。`n.index` 是节点在图里的唯一句柄。

`graph.js:556-584` 挂载时还会再校验一次端点，并在 console 报错；少一条线不影响整张图。

### 标题

`NodeInstance` 只有 `index` 与 `shell_ref`，没有名字：

- 标题：`#${index} · ${shortKind(shell.kind)}`
  - `SYS_CALL_STUB` → `系统`
  - `GENERATED_STUB` → `生成`
  - `CUSTOM_GRAPH` → `图`
  - `COMPOSITE_GRAPH` → `复合`
  - 其它 → 原枚举名
- `title` 属性：`kind · runtime_id · guid`，鼠标停在节点上能看到完整标识。

图列表用 `displayName`（后端写的是函数名/图名，`core/src/node/mod.rs:611`），空则退回 `AssetData.name`，再空则 `图 N`。

## 预览窗结构

`docs/typst/play/index.typ` 里把原来刻意留空的 `.play-preview` 换成固定骨架（不再依赖 `:empty`）：

```typ
#html_elem("div", class: "play-preview is-empty")[
  #html_elem("div", class: "preview-message")[待编译]
  #html_elem("div", class: "preview-shell")[
    // 外层：只有图标的视图栏
    #html_elem("nav", class: "preview-rail")[
      #html_elem("button", class: "preview-rail-button", attrs: (
        "type": "button", "data-view": "graph", "data-active": "",
        "title": "图", "aria-label": "图",
      ))[
        #html_elem("i", class: "material-icons")[account_tree]
      ]
      #html_elem("button", class: "preview-rail-button", attrs: (
        "type": "button", "data-view": "struct",
        "title": "结构体", "aria-label": "结构体",
      ))[
        #html_elem("i", class: "material-icons")[data_object]
      ]
    ]
    #html_elem("div", class: "preview-panes")[
      #html_elem("div", class: "preview-graph-view")[
        // 内层：所有图的名称
        #html_elem("nav", class: "preview-graph-list")[
          #html_elem("div", class: "preview-file", id: "play-preview-file")
        ]
        #html_elem("div", class: "preview-graph-canvas", id: "play-graph-canvas")
      ]
      #html_elem("div", class: "preview-struct-view", attrs: ("hidden": ""))[
        #html_elem("div", class: "preview-message")[结构体 · 待完善]
      ]
    ]
  ]
]
```

同一页还要注册 `ui-graph` 用到的脚本与样式（`docs/typst/lib/graph.typ:41-44` 的同一份内容；`template.typ:62-64` 会按内容去重）：

```typ
#custom_element(() => {
    import_script("/lib/js/graph.js");
    import_style("/lib/css/graph.css");
})[]
```

状态 class 只有四个，互斥：

| class | 时机 | 可见内容 |
|---|---|---|
| `is-empty` | 初始；点「编译」时 | 居中的「待编译」 |
| `is-parsing` | 收到产物，解析/建图进行中 | 居中的「解析产物…」 |
| `is-ready` | 全部图的模板都建好 | `.preview-shell` |
| `is-error` | 剥壳、取 schema、解码任一步失败 | 居中的错误原因 |

CSS 骨架（`style.css` 里替换 `:empty` 那一组规则）：

```css
.play-preview { display: flex; }
.play-preview > .preview-shell { display: none; }
.play-preview.is-ready > .preview-shell { display: flex; flex: 1 1 0; min-height: 0; }
.play-preview > .preview-message { margin: auto; text-align: center; }
.play-preview.is-ready > .preview-message { display: none; }
.play-preview.is-empty > .preview-message { font-size: 2em; }
```

「里面的元素隐藏」就落在 `.play-preview > .preview-shell { display:none }` 这一条上：非就绪时整棵交互结构不参与布局，只剩一句状态文字。

## 图视图

### 外层视图栏

两个原生 `<button>`，只有图标没有文字：

- `data-view="graph"` → `account_tree`
- `data-view="struct"` → `data_object`
- 激活项带 `data-active`，样式沿用 `.play-tab[data-active]` 的配色变量。
- Material Icons 字体由页面已有的 Shoelace 引入负责，不需要额外加载。

切换：

- 图：移除结构体页的 `hidden`，给图视图移除 `hidden`；若已就绪但还没挂载，此刻再挂。
- 结构体：反过来。**不要**卸载已挂载的 `ui-graph`——保留它的平移/缩放状态。

### 图列表

`.preview-graph-list` 自己是一列，顶部 `.preview-file` 显示 `rust2genshin_demo.gia · 15.5 KiB`（取代原先那句产物说明），下面是每张图一个按钮：

```html
<button class="preview-graph-item" data-active>
  <span class="preview-graph-name">main</span>
  <span class="preview-graph-count">37</span>
</button>
```

- 所有图都列出来，包括 0 节点的图；0 节点的条目加 `is-empty-graph`，文字变淡，但仍可点——右侧画布显示「这张图没有节点」。
- 默认选中**第一张有节点的图**；都没有节点就选第一张。
- 点选只改 `selected` 并调用 `mountSelected()`。

### 画布与挂载时机

`.preview-graph-canvas` 是 `position: relative; flex: 1; min-width: 0; min-height: 0; overflow: hidden`。挂进去的 `ui-graph` 用页面 CSS 铺满：

```css
.preview-graph-canvas > ui-graph {
    position: absolute;
    inset: 0;
    height: 100%;
    margin: 0;
    border: 0;
    border-radius: 0;
}
```

不写 `data-height`，让 CSS 决定高度；`graph.js:517-518` 只在属性存在时才写内联高度。

**关键约束**：`ui-graph` 在 `connectedCallback` 里量节点尺寸（`graph.js:398-405`、`:407-509`）。祖先 `display:none` 时所有矩形都是 0，挂出来就是一团叠在原点的节点。因此：

- 模板可以提前建（detach，不触发 `connectedCallback`）；
- **只有同时满足**「预览面板可见（`!panels.preview.hidden`）」「当前是图视图」「状态 is-ready」才 `canvas.replaceChildren(template)`；
- 切到预览选项卡时，先 `hidden = false`，再在 `requestAnimationFrame` 里挂载，确保布局已经生效；
- 切走不卸载，回来也不重挂（元素还在，尺寸没变，`ui-graph.done` 会挡住重复构建）；
- 从未挂过的模板在第一次可见时才第一次连接，量到的就是真实尺寸。

备选方案「把两个面板都改成绝对定位 + `visibility`」被否决：它会改动整页面板切换的既有约定（`style.css:619-632`），只为预览服务，代价不成比例。

## 生命周线与状态机

`script.js` 里维护：

```js
const preview = {
    generation: 0,     // 每次产物/每次点编译都 ++
    graphs: [],        // collectGraphs 的结果
    templates: [],     // 与 graphs 同序的 detach <ui-graph>
    selected: 0,
    view: "graph",
};
```

收到产物（改造 `onArtifact`，`script.js:1470-1474`）：

```js
artifactBytes = new Uint8Array(data);
dlBtn.hidden = false;
startPreview(artifactBytes); // 不 await：解析在后台跑，界面先停在 is-parsing
```

原来的 `showArtifactNote`（`script.js:1579-1582`）删除，产物名与体积改由 `.preview-file` 在就绪后显示；`is-parsing` / `is-error` 期间只显示状态文字。

`startPreview`：

1. `const gen = ++preview.generation;`
2. `setPreviewState("is-parsing", "解析产物…")`；
3. `const origin = httpOrigin(client.url)`；`client.url` 为空就进 `is-error`；
4. `const type = await loadSchema(origin)`；
5. `const bundle = decodeBundle(type, bytes)`；`const graphs = collectGraphs(bundle)`；
6. 逐张 `buildGraphElement`，**每张之间 `await new Promise(r => setTimeout(r, 0))`**，让页面有机会响应（图多时不至于长时间不刷新）；每一步先检查 `gen === preview.generation`，过期就直接返回，不碰 DOM；
7. 填图列表、更新 `.preview-file` 的文案与体积；
8. `setPreviewState("is-ready")`；`selected` 默认第一张有节点的图；如果预览选项卡正开着**且当前是图视图**就挂载，否则等用户切到可见时再挂。

点「编译」（`script.js:1529-1540`）时：

- `preview.generation++`（让还在跑的解析作废）；
- `preview.graphs/templates` 清空；画布清空；
- `setPreviewState("is-empty", "待编译")`。

失败：`setPreviewState("is-error", "解析失败：" + err.message)`。**下载按钮照旧可用**——字节本身是好的，预览失败不影响产物。

`setPreviewState(name, message = "")` 只做两件事：把四个状态 class 互斥地换掉、写 `.preview-message` 的文字。`.preview-shell` 的显示与否完全由 CSS 看 class 决定，JS 不直接动它。

## 样式细节

- 外层栏与列表栏都用 `--border` / `--code-bg` / `--muted` 等现有 token，浅色深色都跟着主题走。
- 外层栏宽约 40px，按钮 32×32，图标 20px；`data-active` 用 `--accent` 作为前景色并加一层淡背景。
- `.preview-panes` 是 `display: flex; flex: 1; min-width: 0; min-height: 0`；`.preview-graph-view` 是 `display: flex; flex: 1; min-height: 0`，里面列表栏与画布左右排。
- 图列表栏宽约 180px，`overflow-y: auto`；名称超长省略，`title` 给全名。
- 图视图与结构体视图靠 `hidden` 切换。两处都显式写了 `display: flex`，所以必须各补一条 `[hidden] { display: none }`，否则 `display` 会盖掉 UA 的 `hidden`（和 `.play-panel[hidden]` 同一个坑，`style.css:630-632`）。
- 面板就绪时 `.play-preview` 去掉内边距（`padding: 0`），让栏贴边、画布占满；边框与圆角保留。
- 结构体视图居中一行「结构体 · 待完善」，用 `--muted`。

## 测试

### 后端

在 `playground/src/server/http.rs` 的 `mod tests` 里补两条，沿用现有 `ready_state` / `get` 助手：

1. `proto_is_served_from_the_repo`：临时仓库写入 `core/proto/asset.proto`（内容含 `message AssetBundleData`），`GET /proto/asset.proto` 返回 200、`content-type: text/plain; charset=utf-8`、正文一致。
2. `proto_is_404_when_missing`：文件不存在时返回 404。

命令：`cargo test -p rust2genshin-playground`。

### 前端

仓库没有 JS 测试设施，本功能也不引入。手工验收：

1. `cargo run -p build-demo --bin beyond-server` 产出 `target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia`；
2. 后台挂 `cargo run -p rust2genshin-docs --bin watch`，打开摆弄页；
3. 载入样例、点编译，**停留在「日志」选项卡**：
   - 编译完成后预览仍是隐藏的，切到「预览」应立刻看到图，节点没有叠在原点；
4. 图列表应有主图与所有复合图；主图可拖动节点、右键平移、滚轮缩放、悬停高亮连线；
5. 来回切「日志 / 预览」与「图 / 结构体」：拖动过的位置不丢；
6. 再点一次编译：预览先回到「待编译」，成功后重新出现；
7. 下载按钮全程可用；
8. 断网模拟 schema 取不到：预览显示「解析失败：…」，下载不受影响。

## 文档更新

- 改写 `docs/typst/arch/play.typ:207` 那段「预览面板只有占位」的叙述，补：
  - `/proto/asset.proto` 的用途与来源；
  - 浏览器侧 protobuf.js 解码，服务端不解码；
  - 复用 `ui-graph`、模板 detach 构建 + 可见时才挂载的原因（隐藏祖先量不到尺寸）。
- `CLAUDE.md` 不需要改：它只留指路，预览属于叙述性内容。

## 风险与取舍

1. **隐藏祖先量不到尺寸**：本设计最硬的约束。已用「detach 构建 + 可见时挂载 + 切走不卸载」规避；实现时**不要**在 `is-parsing` 阶段就把 `ui-graph` 插进画布。
2. **proto 与产物版本错位**：protobuf 的默认值语义能兜住绝大多数情况；解析语法错误才会显示失败。若以后 schema 出现破坏性变更，预览会先于编译表现为失败，属于可接受的可观测性。
3. **大图性能**：每张图一套 DOM 模板。demo 量级（十几 KB、几十个节点）没有压力；如果未来产物膨胀，再改成只构建选中图。
4. **引脚没有名字**：本版只有形状和类型 ID。后续可以在同一遍扫描里用 `interface_data` 的 `name` 建 shell 标识 → 名字的表，补上复合调用节点及其引脚的名字，不需要改协议。
5. **`shell_sig.index` 跳号**：已按 `index` 建表，跳号本身不影响解析；被连接引用却没有声明的槽也会补出引脚。真正的盲区是 `port_mapping`（复合图边界）——本版不画外部端口，边只落在图内节点之间。
