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
    CUSTOM_GRAPH: "图",
    COMPOSITE_GRAPH: "复合",
};

/* 节点 ID → 显示名。键是 `shell_ref.runtime_id`，也就是参考项目
 * `utils/node_data/data.json` 里的 `ID`（`SYS_CALL_STUB` 的存根 ID）。
 * 只收本项目常吐出来的那些；没列的按 `syscall#<id>` 显示，一眼能看出
 * 是漏配了名字而不是名字本身就叫这个。
 *
 * 名字取的是游戏内 `zh-Hans` 名,和编辑器里看到的一致;新增时照抄即可,
 * 不要自己意译。 */
const NODE_NAMES = {
    // 控制流 / 执行
    1: "打印字符串",
    2: "双分支",
    3: "多分支",
    5: "有限循环",
    6: "跳出循环",
    77: "结算关卡",
    178: "激活/关闭实体布设组",
    179: "查询当前激活的实体布设组列表",
    190: "转发事件",
    // 变量
    18: "获取局部变量",
    19: "设置局部变量",
    22: "设置自定义变量",
    36: "自定义变量变化时",
    50: "获取自定义变量",
    3360: "查询自定义变量快照",
    // 算术 / 比较
    14: "是否相等",
    169: "拼装列表",
    180: "数据类型转换",
    200: "加法运算",
    202: "减法运算",
    204: "乘法运算",
    206: "除法运算",
    208: "模运算",
    209: "幂运算",
    211: "取较大值",
    213: "取较小值",
    215: "对数运算",
    216: "绝对值运算",
    218: "取符号运算",
    221: "算术平方根运算",
    222: "范围限制运算",
    224: "取整数运算",
    475: "枚举是否相等",
    // 逻辑
    226: "逻辑与运算",
    227: "逻辑或运算",
    228: "逻辑异或运算",
    229: "逻辑非运算",
    230: "数值小于",
    231: "数值小于等于",
    232: "数值大于",
    233: "数值大于等于",
    // 三维向量 / 常量
    9: "拆分三维向量",
    10: "三维向量加法",
    11: "三维向量减法",
    12: "三维向量缩放",
    13: "三维向量夹角",
    74: "三维向量归一化",
    191: "圆周率",
    192: "三维向量：零向量",
    193: "三维向量：上方",
    194: "三维向量：下方",
    195: "三维向量：左侧",
    196: "三维向量：右侧",
    197: "三维向量：前方",
    198: "三维向量：后方",
    220: "三维向量模运算",
    225: "创建三维向量",
    474: "三维向量旋转",
    // 实体 / 触发器
    69: "销毁实体",
    70: "创建实体",
    71: "实体创建时",
    72: "实体移除/销毁时",
    73: "获取自身实体",
    75: "以GUID查询实体",
    76: "以实体查询GUID",
    99: "获取实体位置与旋转",
    // 列表
    100: "拼接列表",
    107: "清除列表",
    114: "列表是否包含该值",
    121: "查找列表并返回值的序号",
    128: "获取列表对应值",
    135: "对列表插入值",
    142: "获取列表长度",
    149: "获取列表最大值",
    151: "获取列表最小值",
    153: "对列表移除值",
    160: "对列表修改值",
    167: "列表排序",
    // 字典
    948: "对字典设置或新增键值对",
    1158: "以键查询字典值",
    1298: "以键对字典移除键值对",
    1368: "查询字典是否包含特定键",
    1438: "查询字典是否包含特定值",
    1508: "获取字典中键组成的列表",
    1578: "获取字典中值组成的列表",
    1648: "查询字典长度",
    1718: "清空字典",
    1928: "对字典按键排序",
    1938: "对字典按值排序",
    // 定时器 / 运动器 / 碰撞 / 特效
    79: "启动定时器",
    80: "暂停定时器",
    81: "恢复定时器",
    82: "终止定时器",
    83: "定时器触发时",
    84: "添加匀速直线型基础运动器",
    85: "添加匀速旋转型基础运动器",
    86: "停止并删除基础运动器",
    87: "暂停基础运动器",
    88: "恢复基础运动器",
    89: "基础运动器停止时",
    90: "激活/关闭碰撞触发器",
    91: "离开碰撞触发器时",
    92: "进入碰撞触发器时",
    93: "播放限时特效",
    94: "挂载循环特效",
    95: "清除循环特效",
    473: "根据特效资产清除特效",
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

/* 资产标识符的完整键。同一个 guid 可能出现在不同 Category / AssetKind
 * 下，只比 guid 会拿错资产 —— 四个字段都得对上（缺省字段按 proto 默认值
 * 归一化）。 */
function identKey(id) {
    return [
        id.source ?? "UNKNOWN_SOURCE",
        id.category ?? "UNKNOWN_CATEGORY",
        id.kind ?? "BASIC",
        String(id.guid ?? ""),
    ].join("|");
}

/** 收集所有 node decl 资产的名称，键是**完整** Identifier。
 *
 * 被调用的复合节点在图里只是一个 GENERATED_STUB，`shell_ref.runtime_id`
 * 存的是 decl 的 guid；按 `NODE_DECL` + `BASIC` 把整个标识符拼出来再去
 * 找资产，不能只拿 guid 撞。 */
export function collectNodeDecls(bundle) {
    const out = new Map();
    for (const asset of [...(bundle.assets ?? []), ...(bundle.dependencies ?? [])]) {
        const id = asset.id;
        if (!id || id.category !== "NODE_DECL") continue;
        if ((id.kind ?? "BASIC") !== "BASIC") continue;
        const name = asset.name || asset.interfaceData?.inner?.interface?.name;
        if (name) out.set(identKey(id), name);
    }
    return out;
}

/**
 * 建一张 detach 的 <ui-graph>：light DOM 里只有 ui-graph-node / pin /
 * connection 三种数据载体，渲染全交给 ui-graph。
 *
 * 产物里的连接和界面上看到的引脚并不一一对应，这是这里最绕的一段：
 *   - `IN_CONTROL` **根本不进 proto**（`core/src/node/mod.rs` 只发
 *     controls_out / values_out / values_in），控制边的输入那头只以
 *     `target_pin_shell` 的形式出现。
 *   - 没有 selector 的槽也不发引脚 —— 字面量、取变量这类节点的输出
 *     就是这样，别的引脚照样能连到它。
 *   所以引脚要按「声明过的 + 被边引用过的」取并集，缺的那部分补出来。
 *   - 数据边在**输入、输出两端各记一次**（同一根线写了两条
 *     `NodeConnection`），画之前要按两端去重，否则每条数据边都叠两层。
 */
export function buildGraphElement(graph, decls = new Map()) {
    const el = document.createElement("ui-graph");
    const nodes = graph.node ?? [];
    const records = new Map();

    // 第一遍只扫边：记下每根边两端的引脚，用来补齐没声明的引脚。
    const referenced = new Map(); // `node:dir-kind` -> Set<shell index>
    const need = (node, dir, kind, index) => {
        const key = `${node}:${dir}-${kind}`;
        let set = referenced.get(key);
        if (!set) referenced.set(key, (set = new Set()));
        set.add(index ?? 0);
    };
    forEachEdge(nodes, (edge) => {
        need(edge.fromNode, "out", edge.kind, edge.fromSig.index);
        need(edge.toNode, "in", edge.kind, edge.toSig.index);
    });

    // 复合图的导出端口也要标。映射里「内部」两端才是图里真实存在的引脚：
    // external_port 只是外壳上的编号（内容只写 index，extern[] 前缀由
    // ui-graph 渲染时补），internal_target_node_handle + internal_port_shell
    // 才是节点上的那根。内部控制入口在产物里不单独发引脚，这里一并记进
    // referenced，下面补出来。
    const externs = new Map(); // node -> [{ dir, kind, index, text }]
    for (const m of graph.portMapping ?? []) {
        const ext = m.externalPort;
        const inner = m.internalPortShell;
        const target = m.internalTargetNodeHandle;
        if (!ext || !inner || target == null) continue;
        const type = PIN_KINDS[inner.kind];
        if (!type) continue;
        const [dir, kind] = type;
        const index = inner.index ?? 0;
        need(target, dir, kind, index);
        const list = externs.get(target) ?? [];
        list.push({ dir, kind, index, text: String(ext.index ?? 0) });
        externs.set(target, list);
    }

    for (const n of nodes) {
        const node = document.createElement("ui-graph-node");
        node.id = `n${n.index}`;
        node.textContent = nodeLabel(n, decls);
        node.title = `#${n.index}`;
        // 系统调用按控制流形态分三类上色。复合节点/图形节点保持默认色。
        if ((n.shellRef ?? {}).kind === "SYS_CALL_STUB") {
            node.dataset.role = syscallRole(n, referenced);
        }

        const groups = pinGroups(n.pins ?? []);
        for (const key of ["in-ctrl", "out-ctrl", "in-data", "out-data"]) {
            const [dir, kind] = key.split("-");
            const declared = new Map(groups[key].map((p) => [p.index, p]));
            for (const index of referenced.get(`${n.index}:${dir}-${kind}`) ?? []) {
                if (!declared.has(index)) declared.set(index, { index });
            }
            const group = [...declared.values()].sort((a, b) => a.index - b.index);

            const table = new Map();
            group.forEach((p, ordinal) => {
                // 同一组里 shell index 可能跳号（后端只为有 selector 的槽发引脚），
                // 所以序号必须按排序后的位置重新编，不能直接用 shell index。
                table.set(`${dir}-${kind}-${p.index}`, ordinal);
                const pin = document.createElement("pin");
                pin.dataset.dir = dir;
                pin.dataset.kind = kind;
                if (p.typeId != null) pin.dataset.type = String(p.typeId);
                node.appendChild(pin);
            });
            records.set(`${n.index}:${dir}-${kind}`, table);
        }

        // 带默认值的输入引脚：TypedValue.is_set 为真就是用户/后端给了值。
        // 值要格式化成人能读的一小串，摆在节点外侧。
        const inData = records.get(`${n.index}:in-data`);
        for (const p of n.pins ?? []) {
            const sig = p.shellSig ?? {};
            if (sig.kind !== "IN_VALUE" || p.value?.isSet !== true) continue;
            const ordinal = inData?.get(`in-data-${sig.index ?? 0}`);
            if (ordinal == null) continue;
            // 多态引脚只要选了类型 is_set 就是真,值却可能来自连线而非默认值
            // （此时 actual_value 里没有 storage）。格式化不出东西就不标。
            const text = formatTypedValue(p.value);
            if (!text) continue;
            el.appendChild(pinInfo("default", n.index, "in", "data", ordinal, text));
        }

        // 复合图导出的端口：标在它映射到的内部引脚旁。
        for (const x of externs.get(n.index) ?? []) {
            const table = records.get(`${n.index}:${x.dir}-${x.kind}`);
            const ordinal = table?.get(`${x.dir}-${x.kind}-${x.index}`);
            if (ordinal == null) continue;
            el.appendChild(pinInfo("extern", n.index, x.dir, x.kind, ordinal, x.text));
        }

        el.appendChild(node);
    }

    // 第二遍画边。数据边两端各记一次，按「输出端 → 输入端」的键去重。
    const seen = new Set();
    forEachEdge(nodes, (edge) => {
        const from = pinOrdinal(
            records.get(`${edge.fromNode}:out-${edge.kind}`), "out", edge.kind, edge.fromSig);
        const to = pinOrdinal(
            records.get(`${edge.toNode}:in-${edge.kind}`), "in", edge.kind, edge.toSig);
        if (from == null || to == null) return;
        const key = `${edge.fromNode}:${from}->${edge.toNode}:${to}:${edge.kind}`;
        if (seen.has(key)) return;
        seen.add(key);
        const conn = document.createElement("connection");
        conn.dataset.kind = edge.kind;
        conn.dataset.fromNode = `n${edge.fromNode}`;
        conn.dataset.fromIndex = String(from);
        conn.dataset.toNode = `n${edge.toNode}`;
        conn.dataset.toIndex = String(to);
        el.appendChild(conn);
    });
    return el;
}

/**
 * 把 `PinData.connection` 统一成「输出端 → 输入端」。
 * 载体是输出引脚时它自己就是源；载体是输入引脚时 `target_pin_shell`
 * 才是源（后端在 values_in 上也记了一份）。
 */
function forEachEdge(nodes, cb) {
    for (const n of nodes) {
        for (const p of n.pins ?? []) {
            const carrier = PIN_KINDS[p.shellSig?.kind];
            if (!carrier) continue;
            const [dir, kind] = carrier;
            for (const c of p.connection ?? []) {
                const target = c.targetPinShell;
                if (!target || !PIN_KINDS[target.kind]) continue;
                if (dir === "out") {
                    cb({
                        fromNode: n.index, fromSig: p.shellSig, kind,
                        toNode: c.targetNodeIndex, toSig: target,
                    });
                } else {
                    cb({
                        fromNode: c.targetNodeIndex, fromSig: target, kind,
                        toNode: n.index, toSig: p.shellSig,
                    });
                }
            }
        }
    }
}

function pinGroups(pins) {
    const groups = {
        "in-ctrl": [],
        "out-ctrl": [],
        "in-data": [],
        "out-data": [],
    };
    for (const p of pins) {
        const sig = p.shellSig ?? {};
        const type = PIN_KINDS[sig.kind];
        if (!type) continue;
        const [dir, kind] = type;
        groups[`${dir}-${kind}`].push({ index: sig.index ?? 0, typeId: p.type });
    }
    return groups;
}

/** 引脚签名 → 该组里的 DOM 序号；签名不属于这一组时返回 null。 */
function pinOrdinal(table, dir, kind, sig) {
    if (!table || !sig) return null;
    const type = PIN_KINDS[sig.kind];
    if (!type || type[0] !== dir || type[1] !== kind) return null;
    return table.get(`${dir}-${kind}-${sig.index ?? 0}`) ?? null;
}

/** `<ui-graph>` 直接子节点上的引脚信息标。`index` 是组内序号，和
 * `connection` 两端用的是同一套号。 */
function pinInfo(kind, node, dir, pinKind, index, text) {
    const el = document.createElement("pin-info");
    el.setAttribute("kind", kind);
    el.dataset.node = `n${node}`;
    el.dataset.dir = dir;
    el.dataset.kind = pinKind;
    el.dataset.index = String(index);
    el.textContent = text;
    return el;
}

/** `TypedValue` 的 oneof storage → 一小串可读文本。多态值递归取实际值；
 * 结构体没有字段名（proto 侧只有 repeated field），按顺序排列即可 ——
 * 这里只做预览，不是给机器读的。 */
function formatTypedValue(v) {
    if (!v) return "";
    if (v.valPoly) return formatTypedValue(v.valPoly.actualValue);
    if (v.valInt) return String(v.valInt.value ?? 0);
    if (v.valFloat) return String(v.valFloat.value ?? 0);
    if (v.valString) return JSON.stringify(v.valString.value ?? "");
    if (v.valEnum) return String(v.valEnum.value ?? 0);
    if (v.valId) return String(v.valId.value ?? 0);
    if (v.valVector) {
        const { x = 0, y = 0, z = 0 } = v.valVector.value ?? {};
        return `(${x}, ${y}, ${z})`;
    }
    if (v.valList) return `[${(v.valList.element ?? []).map(formatTypedValue).join(", ")}]`;
    if (v.valStruct) return `{${(v.valStruct.field ?? []).map(formatTypedValue).join(", ")}}`;
    if (v.valMap) {
        return `{${(v.valMap.pairs ?? [])
            .map((p) => `${formatTypedValue(p.key)}: ${formatTypedValue(p.value)}`)
            .join(", ")}}`;
    }
    if (v.valPair) return `(${formatTypedValue(v.valPair.key)}, ${formatTypedValue(v.valPair.value)})`;
    return "";
}

/** 节点正面显示的名字。系统调用查 `NODE_NAMES`，查不到就露出 ID ——
 * 这样漏配的名字不会伪装成一个正常节点。
 *
 * 生成的存根对应一个 node decl 资产：`shell_ref.runtime_id` 是 decl 的
 * guid，用完整标识符（NODE_DECL + BASIC + guid）去 `decls` 里找它的
 * 名字。复合节点因此在调用点显示成真正的名字，而不是笼统的"生成"；
 * 找不到时保留 id，一眼能看出缺了什么。 */
/* 系统调用的控制流形态:
 *   - `pure`  既没有控制入也没有控制出(数学、查询这类纯数据节点)
 *   - `event` 只有控制出(事件触发点)
 *   - `exec`  有控制入 —— 包括只有控制入的汇聚点和进出都有的执行节点
 * 引脚声明里缺的靠边引用补:后端只为有 selector 的槽发引脚,没声明的
 * 控制口只有连了线才会出现在 referenced 里。 */
function syscallRole(n, referenced) {
    const has = (dir) =>
        (referenced.get(`${n.index}:${dir}-ctrl`)?.size ?? 0) > 0 ||
        (n.pins ?? []).some((p) => PIN_KINDS[p.shellSig?.kind]?.[0] === dir &&
            PIN_KINDS[p.shellSig?.kind]?.[1] === "ctrl");
    if (has("in")) return "exec";
    return has("out") ? "event" : "pure";
}

function nodeLabel(n, decls) {
    const shell = n.shellRef ?? {};
    if (shell.kind === "SYS_CALL_STUB") {
        return NODE_NAMES[shell.runtimeId] ?? `syscall#${shell.runtimeId ?? "?"}`;
    }
    if (shell.kind === "GENERATED_STUB") {
        const id = { category: "NODE_DECL", kind: "BASIC", guid: shell.runtimeId };
        return decls.get(identKey(id)) ?? `复合节点#${shell.runtimeId ?? "?"}`;
    }
    return SHELL_LABELS[shell.kind] ?? "节点";
}
