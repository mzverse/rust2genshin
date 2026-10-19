# 摆弄页 `.gia` 预览实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 摆弄页收到 `.gia` 后在浏览器里异步解码，并用文档站已有的 `<ui-graph>` 按图浏览节点与连线。

**Architecture:** 后端只加一条把仓库里的 `core/proto/asset.proto` 原样读出去的路由；前端新增 `play/gia.js` 负责剥壳、protobuf.js 解码、收集图、构建 detach 的 `ui-graph`；`script.js` 管状态 class、双层侧栏与「可见时才挂载」；面板骨架与样式落在 `index.typ` / `style.css`。

**Tech Stack:** Rust + axum（playground）、protobuf.js 7.4.0（esm.sh 动态 import）、原生 Web Components（`ui-graph`）、Typst html 输出。

**Spec:** `docs/superpowers/specs/2026-10-19-play-gia-preview-design.md`

## Global Constraints

- **不使用 subagent。** 全部工作在本会话内完成。
- 所有文件操作使用完整的 Windows 绝对路径（`C:\projects\rust2genshin\...`）。
- 不自动 `git commit` / `git push`；用户明确要求时才提交。禁止 `git add .` / `git add -A`。
- 不改动：`.idea/dictionaries/project.xml`、`CLAUDE_alt.md`、`docs/superpowers/specs/2026-09-04-cast-design.md`、`export.gia*`、`.superpowers/`。
- 文档站调试只挂 `cargo run -p rust2genshin-docs --bin watch`，不跑 `--bin compile`，不另起静态服务器。
- 预览失败**不影响**下载；`artifactBytes` 与下载按钮的逻辑保持原样。
- 所有面向用户的文案用中文；代码标识符保持原样。

## Review Focus

实现与验收时逐条盯住这五类输入：

1. **产物到达时预览选项卡不可见**（用户停在「日志」）：不能提前挂载 `ui-graph`，否则尺寸量成 0。
2. **解析还在跑时又点编译**：旧的那次必须整体作废，不能把旧图写进新界面。
3. **空图 / 没有连线的图**：列表里能选中，画布给占位，不抛异常。
4. **`.gia` 头损坏或 `/proto/asset.proto` 取不到**：进 `is-error`，下载仍可用。
5. **引脚组内顺序与 `shell_sig.index` 不一致、或连接指向被过滤的引脚**：边要么落在正确引脚，要么被跳过，不能让整张图挂掉。

每个 Task 的验证步骤会逐条覆盖它负责的那几项。

---

### Task 1: 后端 `/proto/asset.proto` 路由

**Files:**
- Modify: `C:\projects\rust2genshin\playground\src\server\http.rs`（`sample_path` 旁、`router`、`mod tests`）

**Interfaces:**
- Produces: `pub fn proto_path(cfg: &Config) -> PathBuf`；HTTP `GET /proto/asset.proto` → `200 text/plain; charset=utf-8`，文件缺失 → `404`。

- [ ] **Step 1: 写失败测试**

在 `http.rs` 的 `mod tests` 里、`sample_is_404_when_the_file_is_gone` 之后加：

```rust
    #[tokio::test]
    async fn proto_is_served_from_the_repo() {
        let d = tempfile::tempdir().unwrap();
        let proto = d.path().join("core/proto/asset.proto");
        std::fs::create_dir_all(proto.parent().unwrap()).unwrap();
        let body = b"package rust2genshin;\nmessage AssetBundleData {}\n";
        std::fs::write(&proto, body).unwrap();
        let res = get(ready_state(d.path()).await, "/proto/asset.proto").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], body);
    }

    #[tokio::test]
    async fn proto_is_404_when_missing() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/proto/asset.proto").await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
```

- [ ] **Step 2: 跑测试，确认失败**

Run: `cargo test -p rust2genshin-playground proto_is_ -- --nocapture`
Expected: FAIL —— `/proto/asset.proto` 落到 404（路由不存在）。

- [ ] **Step 3: 实现**

在 `sample_path` 之后加：

```rust
/// 浏览器解析 `.gia` 用的 schema。跟着仓库工作区走，不 embed ——
/// 与 `/sample` 同一个理由：部署的就是当前仓库，进程启动时冻结反而会漂。
pub fn proto_path(cfg: &Config) -> PathBuf {
    cfg.repo_dir.join("core/proto/asset.proto")
}
```

在 `sample` 处理函数之后加：

```rust
async fn asset_proto(State(st): State<Arc<AppState>>) -> Response {
    let Ok(bytes) = tokio::fs::read(proto_path(&st.cfg)).await else {
        return (StatusCode::NOT_FOUND, "asset.proto 不在仓库里").into_response();
    };
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], bytes).into_response()
}
```

在 `router()` 的 `.route("/sample", get(sample))` 后加：

```rust
        .route("/proto/asset.proto", get(asset_proto))
```

- [ ] **Step 4: 跑测试，确认通过**

Run: `cargo test -p rust2genshin-playground`
Expected: 全绿，包含原有的 `/sample`、CORS、webhook 测试。

---

### Task 2: 前端数据层 `play/gia.js`

> **执行修订（已在实现时落实）**：下面的 `pinPlan` 假定四类引脚都在 `PinData` 里、连接只挂在输出引脚上，实测都不成立：`IN_CONTROL` 不进 proto；没有 selector 的槽（字面量等）不发输出引脚；数据边在两端各记一次。最终实现改成「声明引脚 ∪ 边引用引脚」，并按两端去重；细节见 `gia.js` 的 `buildGraphElement` 注释与设计稿「引脚顺序」一节。本任务其余部分（剥壳、schema、解码、收集图）不变。

**Files:**
- Create: `C:\projects\rust2genshin\docs\typst\play\gia.js`

**Interfaces:**
- Produces（供 Task 4 使用）：
  - `unwrapGia(bytes: Uint8Array): Uint8Array`
  - `loadSchema(origin: string): Promise<object>`（protobuf Type，按 origin 缓存）
  - `decodeBundle(type, bytes): object`（`toObject` 后的普通对象）
  - `collectGraphs(bundle): Array<{ name: string, nodeCount: number, graph: object }>`
  - `buildGraphElement(graph): HTMLElement`（detach 的 `<ui-graph>`）

- [ ] **Step 1: 新建文件，写入完整实现**

```js
/* `.gia` 预览的数据层：剥壳、schema、解码、收集图、构建 <ui-graph>。
 *
 * 这里只碰数据与元素结构，不碰预览面板的布局和状态 —— 那些在 script.js。
 * `buildGraphElement` 返回的元素**从不插入文档**，所以 ui-graph 的
 * connectedCallback 不会在隐藏的祖先下量尺寸；什么时候挂由 script.js 决定。
 */

const HEADER_BYTES = 20;
const TAIL_BYTES = 4;
const HEADER_SCHEMA = 1;
const HEADER_MAGIC = 0x0326;
const FILE_TYPE_ASSET_BUNDLE = 3;
const TAIL_MAGIC = 0x0679;
const PROTOBUF_URL = "https://esm.sh/protobufjs@7.4.0";

/* PinSignature.Kind（core/proto/asset.proto:800-824）→ ui-graph 的
 * dir / kind。toObject 用了 enums: String，所以拿到的是枚举名。 */
const PIN_KINDS = {
    IN_CONTROL: ["in", "ctrl"],
    OUT_CONTROL: ["out", "ctrl"],
    IN_VALUE: ["in", "data"],
    OUT_VALUE: ["out", "data"],
};

const SHELL_LABELS = {
    SYS_CALL_STUB: "系统",
    GENERATED_STUB: "生成",
    CUSTOM_GRAPH: "图",
    COMPOSITE_GRAPH: "复合",
};

/** 剥掉 20 字节头与 4 字节尾；任何一项校验不过都直接报错。 */
export function unwrapGia(bytes) {
    if (bytes.length < HEADER_BYTES + TAIL_BYTES) {
        throw new Error("文件太短，不是 .gia");
    }
    const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const total = v.getUint32(0);
    const schema = v.getUint32(4);
    const magic = v.getUint32(8);
    const fileType = v.getUint32(12);
    const len = v.getUint32(16);
    const tail = v.getUint32(bytes.length - 4);
    if (
        total !== bytes.length - 4 ||
        schema !== HEADER_SCHEMA ||
        magic !== HEADER_MAGIC ||
        fileType !== FILE_TYPE_ASSET_BUNDLE ||
        len !== bytes.length - HEADER_BYTES - TAIL_BYTES ||
        tail !== TAIL_MAGIC
    ) {
        throw new Error("文件头校验失败，不是 .gia");
    }
    return bytes.subarray(HEADER_BYTES, bytes.length - TAIL_BYTES);
}

let schemaCache = null; // { origin, type }

/** 从当前后端取 schema 并解析。按 origin 缓存：BACKENDS 可能切后端。 */
export async function loadSchema(origin) {
    if (schemaCache?.origin === origin) return schemaCache.type;
    const res = await fetch(`${origin}/proto/asset.proto`);
    if (!res.ok) throw new Error(`取 schema 失败（HTTP ${res.status}）`);
    const text = await res.text();
    const protobuf = await import(PROTOBUF_URL);
    const { root } = protobuf.parse(text);
    const type = root.lookupType("rust2genshin.AssetBundleData");
    schemaCache = { origin, type };
    return type;
}

/** 解出一份普通对象。longs 用字符串避免 int64 精度问题。 */
export function decodeBundle(type, bytes) {
    const payload = unwrapGia(bytes);
    return type.toObject(type.decode(payload), {
        longs: String,
        enums: String,
        defaults: false,
    });
}

/** 主资源在前，依赖在后；只收带 graph_data 的资源。 */
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

/**
 * 建一张 detach 的 <ui-graph>：light DOM 里只有 ui-graph-node / pin /
 * connection 三种数据载体，渲染全交给 ui-graph。
 */
export function buildGraphElement(graph) {
    const el = document.createElement("ui-graph");
    const records = new Map();

    for (const n of graph.node ?? []) {
        const node = document.createElement("ui-graph-node");
        node.id = `n${n.index}`;
        node.textContent = nodeTitle(n);
        node.title = nodeDetail(n);

        const { plan, table } = pinPlan(n.pins ?? []);
        for (const p of plan) {
            const pin = document.createElement("pin");
            pin.dataset.dir = p.dir;
            pin.dataset.kind = p.kind;
            if (p.typeId != null) pin.dataset.type = String(p.typeId);
            node.appendChild(pin);
        }

        records.set(n.index, { el: node, table });
        el.appendChild(node);
    }

    for (const n of graph.node ?? []) {
        const record = records.get(n.index);
        if (!record) continue;
        for (const p of n.pins ?? []) {
            const from = pinOrdinal(record, "out", p.shellSig);
            if (!from) continue; // connection 只挂在输出引脚上
            for (const c of p.connection ?? []) {
                const target = records.get(c.targetNodeIndex);
                const to = pinOrdinal(target, "in", c.targetPinShell);
                if (!target || !to) continue;
                const conn = document.createElement("connection");
                conn.dataset.kind = from.kind;
                conn.dataset.fromNode = record.el.id;
                conn.dataset.fromIndex = String(from.ordinal);
                conn.dataset.toNode = target.el.id;
                conn.dataset.toIndex = String(to.ordinal);
                el.appendChild(conn);
            }
        }
    }
    return el;
}

/**
 * 四组引脚各排各的：组内按 shell_sig.index 升序，再按 DOM 顺序从 0 编号。
 * 返回 `plan`（DOM 顺序）与 `table`（`dir-kind-index` → DOM 序号）。
 * dir 必须进键：输入和输出各有一根 `data-0`。
 */
function pinPlan(pins) {
    const groups = new Map();
    for (const p of pins) {
        const sig = p.shellSig ?? {};
        const type = PIN_KINDS[sig.kind];
        if (!type) continue;
        const [dir, kind] = type;
        const key = `${dir}-${kind}`;
        let group = groups.get(key);
        if (!group) groups.set(key, (group = []));
        group.push({ dir, kind, index: sig.index ?? 0, typeId: p.type });
    }

    const plan = [];
    const table = new Map();
    for (const key of ["in-ctrl", "out-ctrl", "in-data", "out-data"]) {
        const group = groups.get(key) ?? [];
        group.sort((a, b) => a.index - b.index);
        group.forEach((p, ordinal) => {
            plan.push(p);
            table.set(`${p.dir}-${p.kind}-${p.index}`, ordinal);
        });
    }
    return { plan, table };
}

function pinOrdinal(record, dir, sig) {
    if (!record || !sig) return null;
    const type = PIN_KINDS[sig.kind];
    if (!type || type[0] !== dir) return null;
    const kind = type[1];
    const ordinal = record.table.get(`${dir}-${kind}-${sig.index ?? 0}`);
    return ordinal == null ? null : { kind, ordinal };
}

function nodeTitle(n) {
    const shell = n.shellRef ?? {};
    return `#${n.index} · ${SHELL_LABELS[shell.kind] ?? shell.kind ?? "节点"}`;
}

function nodeDetail(n) {
    const shell = n.shellRef ?? {};
    return [
        shell.kind,
        shell.runtimeId != null ? `runtime ${shell.runtimeId}` : null,
        shell.guid != null ? `guid ${shell.guid}` : null,
    ]
        .filter(Boolean)
        .join(" · ");
}
```

- [ ] **Step 2: 用真实产物验证剥壳**

Run:

```shell
node --experimental-default-type=module --input-type=module -e "import('./docs/typst/play/gia.js').then(async (m) => { const fs = await import('node:fs/promises'); const b = new Uint8Array(await fs.readFile('target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia')); const p = m.unwrapGia(b); if (p.length !== b.length - 24) throw new Error('负载长度不对'); console.log('unwrap ok', b.length, '->', p.length); })"
```

Expected: `unwrap ok <总长> -> <总长-24>`。

若产物尚未构建，先跑 `cargo run -p build-demo --bin beyond-server`。

- [ ] **Step 3: 强化坏输入**

Run（把第 9 个字节改掉，期望抛错）：

```shell
node --experimental-default-type=module --input-type=module -e "import('./docs/typst/play/gia.js').then(async (m) => { const fs = await import('node:fs/promises'); const b = new Uint8Array(await fs.readFile('target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia')); b[8] ^= 1; try { m.unwrapGia(b); console.log('BAD: 没有报错'); process.exit(1); } catch (e) { console.log('reject ok:', e.message); } })"
```

Expected: `reject ok: 文件头校验失败，不是 .gia`。

---

### Task 3: 预览窗骨架与样式

**Files:**
- Modify: `C:\projects\rust2genshin\docs\typst\play\index.typ`
- Modify: `C:\projects\rust2genshin\docs\typst\play\style.css`

**Interfaces:**
- Produces（供 Task 4 查询）：`#play-preview-message`、`#play-preview-file`、`#play-graph-canvas`、`.preview-shell`、`.preview-graph-list`、`.preview-graph-view`、`.preview-struct-view`、`.preview-rail-button[data-view]`；状态 class 挂在 `.play-preview` 上。

- [ ] **Step 1: 改 `index.typ` 的预览面板**

把 135-140 行那段（连同「这一格故意留空」的注释）整体替换为：

```typ
        #html_elem("div", class: "play-panel", id: "play-panel-preview", attrs: ("hidden": ""))[
          // 状态由 class 驱动（is-empty / is-parsing / is-ready / is-error）：
          // 非就绪时 .preview-shell 整体隐藏，只剩一句状态文字。不再用 :empty ——
          // 骨架现在是固定的，内容由 script.js 填。
          #html_elem("div", class: "play-preview is-empty")[
            #html_elem("div", class: "preview-message", id: "play-preview-message")[待编译]
            #html_elem("div", class: "preview-shell")[
              // 外层：只有图标的视图栏
              #html_elem("nav", class: "preview-rail")[
                #html_elem("button", type: "button", class: "preview-rail-button", attrs: (
                  "data-view": "graph",
                  "data-active": "",
                  "title": "图",
                  "aria-label": "图",
                ))[
                  #html_elem("i", class: "material-icons")[account_tree]
                ]
                #html_elem("button", type: "button", class: "preview-rail-button", attrs: (
                  "data-view": "struct",
                  "title": "结构体",
                  "aria-label": "结构体",
                ))[
                  #html_elem("i", class: "material-icons")[data_object]
                ]
              ]
              #html_elem("div", class: "preview-panes")[
                #html_elem("div", class: "preview-graph-view")[
                  // 内层：文件信息 + 所有图的名称
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
        ]
```

在页面已有的 `#custom_element(() => { … script.js … })` 之后加 `ui-graph` 的注册（与 `docs/typst/lib/graph.typ:41-44` 内容相同，模板会按内容去重）：

```typ
#custom_element(() => {
    import_script("/lib/js/graph.js");
    import_style("/lib/css/graph.css");
})[]
```

- [ ] **Step 2: 改 `style.css`**

把 680-720 行（`.play-preview` 那一组，含 `:empty` 与 `data-state="artifact"`）整体替换为：

```css
/* ---- 预览：状态与骨架 ----
 *
 * 状态是 `.play-preview` 上的四个互斥 class。非就绪时 `.preview-shell`
 * 整体 display:none —— 交互结构不参与布局，只剩一句居中的状态文字。
 */
.play-preview {
    display: flex;
    flex: 1 1 0;
    min-height: 0;
    overflow: hidden;
    padding: 0;
    color: var(--fg);
    background-color: var(--code-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
}
.play-preview > .preview-message {
    margin: auto;
    padding: 10px 12px;
    text-align: center;
    color: var(--muted);
}
/* 就绪后骨架接管，状态文字必须整块让位 —— 光清空文字它仍占 padding */
.play-preview.is-ready > .preview-message {
    display: none;
}
.play-preview.is-empty > .preview-message {
    font-size: 2em;
}
.play-preview.is-error > .preview-message {
    color: var(--err);
}

.play-preview > .preview-shell {
    display: none;
}
.play-preview.is-ready > .preview-shell {
    display: flex;
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
}

/* 外层：只有图标的图 / 结构体切换 */
.preview-rail {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: 4px;
    padding: 6px;
    border-right: 1px solid var(--border);
}
.preview-rail-button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    cursor: pointer;
}
.preview-rail-button .material-icons {
    font-size: 20px;
}
.preview-rail-button:hover {
    color: var(--fg);
    background-color: var(--border-soft);
}
.preview-rail-button[data-active] {
    color: var(--accent);
    background-color: var(--border-soft);
}
.preview-rail-button:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: 1px;
}

.preview-panes {
    display: flex;
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
}
.preview-graph-view {
    display: flex;
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
}
/* 两处都显式写了 display，必须自己压掉 hidden —— 和 .play-panel[hidden] 同理 */
.preview-graph-view[hidden],
.preview-struct-view[hidden] {
    display: none;
}

/* 内层：所有图的名称 */
.preview-graph-list {
    display: flex;
    flex: none;
    flex-direction: column;
    width: 180px;
    min-height: 0;
    overflow-y: auto;
    border-right: 1px solid var(--border);
    background-color: var(--graph-canvas);
}
.preview-file {
    flex: none;
    padding: 8px 10px;
    overflow: hidden;
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-bottom: 1px solid var(--border-soft);
}
.preview-graph-item {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    padding: 6px 10px;
    border: 0;
    background: none;
    color: var(--fg);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
}
.preview-graph-item:hover,
.preview-graph-item[data-active] {
    background-color: var(--border-soft);
}
.preview-graph-item[data-active] {
    color: var(--accent);
}
.preview-graph-item.is-empty-graph {
    color: var(--muted);
}
.preview-graph-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.preview-graph-count {
    margin-left: auto;
    color: var(--muted);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
}

/* 画布：ui-graph 铺满；不写 data-height，高度交给 CSS */
.preview-graph-canvas {
    position: relative;
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
}
.preview-graph-canvas > ui-graph {
    position: absolute;
    inset: 0;
    height: 100%;
    margin: 0;
    border: 0;
    border-radius: 0;
}
.preview-canvas-note {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--muted);
}

.preview-struct-view {
    display: grid;
    flex: 1 1 0;
    min-height: 0;
    place-items: center;
    color: var(--muted);
}
```

- [ ] **Step 3: 看页面没有被改坏**

Run（后台）: `cargo run -p rust2genshin-docs --bin watch`
打开摆弄页，Expected:

- 预览选项卡显示居中的大号「待编译」；
- 控制台没有报错；日志/预览切换、编译按钮照旧；
- 页面源码里能找到 `play-preview-message` 与两个 `preview-rail-button`。

---

### Task 4: `script.js` 预览控制器

**Files:**
- Modify: `C:\projects\rust2genshin\docs\typst\play\script.js`（顶部 import、`onArtifact`、编译按钮、选项卡/预览区块）

**Interfaces:**
- Consumes: Task 2 的 `loadSchema` / `decodeBundle` / `collectGraphs` / `buildGraphElement`；Task 3 的 DOM id 与 class。
- Produces: `resetPreview()`（编译按钮与产物生命周期共用）。

- [ ] **Step 1: 顶部加 import**

在 `lsp-client.js` 的 import 块之后加：

```js
import { loadSchema, decodeBundle, collectGraphs, buildGraphElement } from "./gia.js";
```

- [ ] **Step 2: `onArtifact` 改为启动解析**

把 1469-1474 行的处理器替换为：

```js
        // 编译产物：裸 `.gia` 字节。存在内存里，同时启动异步预览。
        onArtifact: (data) => {
            artifactBytes = new Uint8Array(data);
            dlBtn.hidden = false;
            startPreview(artifactBytes); // 不 await：界面先停在 is-parsing
        },
```

- [ ] **Step 3: 编译按钮用新的重置函数**

把 1529-1540 行的 `runBtn.addEventListener` 里的：

```js
        artifactBytes = null;
        clearPreview();
```

替换为：

```js
        artifactBytes = null;
        resetPreview();
```

- [ ] **Step 4: 用完整预览控制器替换旧的选项卡/占位区块**

把 1561-1591 行（从 `// ---- 选项卡:日志 / 预览 ----` 到 `tabs.forEach(…)` 结束）整体替换为：

```js
    // ---- 选项卡:日志 / 预览 ----
    // 两个面板都留在 DOM 里,切换只动 `hidden` —— 这样切走再切回来,日志
    // 里已经滚出来的内容还在,不用重跑就能对着报错看代码。
    const tabs = root.querySelectorAll(".play-tab");
    const panels = {
        log: $("play-panel-log"),
        preview: $("play-panel-preview"),
    };

    // ---- 预览:class 状态 + 双层侧栏 + ui-graph ----
    const preview = panels.preview.querySelector(".play-preview");
    const previewMessage = $("play-preview-message");
    const previewFile = $("play-preview-file");
    const previewCanvas = $("play-graph-canvas");
    const graphList = preview.querySelector(".preview-graph-list");
    const graphView = preview.querySelector(".preview-graph-view");
    const structView = preview.querySelector(".preview-struct-view");
    const railButtons = preview.querySelectorAll(".preview-rail-button");

    const PREVIEW_STATES = ["is-empty", "is-parsing", "is-ready", "is-error"];
    const previewState = {
        generation: 0,   // 每次产物 / 每次点编译都 ++，旧解析据此作废
        graphs: [],      // collectGraphs 的结果
        templates: [],   // 与 graphs 同序；空图是 null
        selected: 0,
        mounted: -1,     // 画布当前挂的是第几张图
        view: "graph",
    };

    const setPreviewState = (state, message = "") => {
        preview.classList.remove(...PREVIEW_STATES);
        preview.classList.add(state);
        previewMessage.textContent = message;
    };

    // 挂载的三个前提缺一不可：已就绪、预览选项卡可见、当前是图视图。
    // ui-graph 在 connectedCallback 里量尺寸，祖先 display:none 时量到的全是 0。
    const previewVisible = () => !panels.preview.hidden && previewState.view === "graph";

    const mountSelected = () => {
        if (!preview.classList.contains("is-ready") || !previewVisible()) return;
        const i = previewState.selected;
        const graph = previewState.graphs[i];
        if (!graph || previewState.mounted === i) return;
        previewState.mounted = i;
        if (graph.nodeCount === 0) {
            const note = document.createElement("div");
            note.className = "preview-canvas-note";
            note.textContent = "这张图没有节点";
            previewCanvas.replaceChildren(note);
            return;
        }
        // template 是 detach 的元素：第一次挂进来时才第一次量尺寸。
        previewCanvas.replaceChildren(previewState.templates[i]);
    };

    const fillGraphList = (size) => {
        previewFile.textContent = `${ARTIFACT_NAME} · ${formatBytes(size)}`;
        graphList.querySelectorAll(".preview-graph-item").forEach((el) => el.remove());
        previewState.graphs.forEach((graph, i) => {
            const item = document.createElement("button");
            item.type = "button";
            item.className = "preview-graph-item";
            item.classList.toggle("is-empty-graph", graph.nodeCount === 0);
            item.title = graph.name;
            item.toggleAttribute("data-active", i === previewState.selected);

            const name = document.createElement("span");
            name.className = "preview-graph-name";
            name.textContent = graph.name;
            const count = document.createElement("span");
            count.className = "preview-graph-count";
            count.textContent = String(graph.nodeCount);
            item.append(name, count);

            item.addEventListener("click", () => {
                previewState.selected = i;
                graphList.querySelectorAll(".preview-graph-item")
                    .forEach((el, j) => el.toggleAttribute("data-active", j === i));
                mountSelected();
            });
            graphList.appendChild(item);
        });
    };

    const setView = (view) => {
        previewState.view = view;
        graphView.hidden = view !== "graph";
        structView.hidden = view !== "struct";
        railButtons.forEach((b) =>
            b.toggleAttribute("data-active", b.dataset.view === view));
        if (view === "graph") requestAnimationFrame(mountSelected);
    };
    railButtons.forEach((b) =>
        b.addEventListener("click", () => setView(b.dataset.view)));

    // 点编译立即把上一份产物作废：generation 让在跑的解析停手。
    const resetPreview = () => {
        previewState.generation++;
        previewState.graphs = [];
        previewState.templates = [];
        previewState.selected = 0;
        previewState.mounted = -1;
        previewCanvas.replaceChildren();
        setView("graph");
        setPreviewState("is-empty", "待编译");
    };

    const startPreview = async (bytes) => {
        const gen = ++previewState.generation;
        previewState.graphs = [];
        previewState.templates = [];
        previewState.selected = 0;
        previewState.mounted = -1;
        previewCanvas.replaceChildren();
        setView("graph");
        setPreviewState("is-parsing", "解析产物…");
        try {
            if (!client.url) throw new Error("还没连上后端");
            const origin = httpOrigin(client.url);
            const type = await loadSchema(origin);
            if (gen !== previewState.generation) return;
            const bundle = decodeBundle(type, bytes);
            const graphs = collectGraphs(bundle);
            const templates = [];
            for (const graph of graphs) {
                if (gen !== previewState.generation) return;
                templates.push(
                    graph.nodeCount > 0 ? buildGraphElement(graph.graph) : null);
                // 让出主线程：图多时不至于整页卡住。
                await new Promise((resolve) => setTimeout(resolve, 0));
            }
            if (gen !== previewState.generation) return;
            previewState.graphs = graphs;
            previewState.templates = templates;
            const first = graphs.findIndex((g) => g.nodeCount > 0);
            previewState.selected = first >= 0 ? first : 0;
            fillGraphList(bytes.length);
            setPreviewState("is-ready");
            requestAnimationFrame(mountSelected);
        } catch (err) {
            if (gen !== previewState.generation) return;
            // 产物本身是好的，下载不受影响；只是预览画不出来。
            setPreviewState("is-error", `解析失败：${err.message}`);
        }
    };

    tabs.forEach((tab) => {
        tab.addEventListener("click", () => {
            const want = tab.dataset.tab;
            tabs.forEach((t) => t.toggleAttribute("data-active", t === tab));
            for (const [name, panel] of Object.entries(panels)) {
                panel.hidden = name !== want;
            }
            // hidden 先落到 DOM 上，下一帧再挂载，尺寸才是真的。
            if (want === "preview") requestAnimationFrame(mountSelected);
        });
    });
```

- [ ] **Step 5: 静态检查**

Run: `node --check docs/typst/play/script.js`
Expected: 无输出（本机 Node 24 默认开启 module syntax detection，没有 `package.json` 也能按 ESM 解析；`--experimental-default-type` 在 24 里已被移除，别用）。

- [ ] **Step 6: 浏览器验收（对应 Review Focus 1、2、3、4）**

先 `cargo run -p build-demo --bin beyond-server`，再在 `cargo run -p rust2genshin-docs --bin watch` 打开的摆弄页里：

1. 载入样例 → 点编译，**停留在「日志」**。编译完成后切到「预览」：图应有正常布局，节点不能叠在左上角。
2. 切回「图 / 结构体」再切回：拖动过的节点位置保留。
3. 再点一次编译：预览立刻回到「待编译」；若在解析途中点，旧结果不能覆盖新界面。
4. 选中 0 节点的图：画布显示「这张图没有节点」，不报错。
5. 下载按钮始终可用。

---

### Task 5: 文档与最终验收

**Files:**
- Modify: `C:\projects\rust2genshin\docs\typst\arch\play.typ:207`

**Interfaces:**
- Consumes: Task 1-4 的最终行为。

- [ ] **Step 1: 改写站点叙述**

把 `arch/play.typ` 第 207 行那段（`*产物只在两头的内存里*` 整段）替换为：

```typ
*产物只在两头的内存里*：后端用一条二进制帧（裸 `.gia` 字节，socket 上设了 `binaryType = "arraybuffer"`）把它送过来，前端存进 `artifactBytes`，「下载」按钮做的事是 `Blob` + 临时 `<a download>`。没有下载链接、也没有第二次 HTTP 往返。点「编译」会把上一份产物*立刻作废* —— 留着它，一次编译失败之后用户点下载拿到的还是上一次的图，而界面上看不出来。

*预览在浏览器里解码*：收到产物后前端异步取同一个后端的 `/proto/asset.proto`（路由只 `tokio::fs::read` 仓库里那份文件，不 embed），用 protobuf.js 解析 schema 并解开 `.gia` 的 proto 负载。服务端不参与解码。解析完把每张图构建成一个 detach 的 `<ui-graph>` 模板（与站点其它节点图共用同一套自定义元素），再按需要挂进画布。挂载时机不是「解析完」而是「预览选项卡可见且当前是图视图」：`ui-graph` 在 `connectedCallback` 里量节点尺寸，祖先 `display:none` 时量到的全是 0。切走不卸载，所以平移、缩放、拖动的位置都留着。预览失败只影响预览，下载照常可用。
```

- [ ] **Step 2: 端到端验收（对应 Review Focus 5）**

1. 打开主图，确认：控制流边是 `ctrl`、数据边是 `data`；边的两端落在正确的引脚上（不只落在节点上）。
2. 图列表里的名称与产物里的 `display_name` 一致；空图计数为 0。
3. 浏览器控制台没有 `graph: 连接引用了不存在的节点` / `引脚不存在`（有少量跳过可以接受，但要知道原因）。
4. 断开后端或伪造一个坏 `.gia`：预览显示「解析失败：…」，下载按钮仍可用。

- [ ] **Step 3: 回归**

Run: `cargo test -p rust2genshin-playground`
Run: `cargo build -p rust2genshin-docs`
Expected: 都通过。

- [ ] **Step 4: 等用户验收**

把改动清单和验收结果报给用户，**不要**自行 commit / push；用户明确要求后再提交。
