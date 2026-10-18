/* lsp-client.js —— CodeMirror 与 LSP 协议的薄桥。
 *
 * 服务端按 Task 6 的协议把 RA 的事件/响应拆成 `LspTokens` / `LspDiagnostic`
 * / `LspCompletion` 透传过来。前端拿到的是 RAW 数据:
 *   - `LspTokens.data` 是 RA `semanticTokens/full` 的 delta 数组,索引值是
 *     LSP 标准 `tokenTypes` 的顺序 —— 与 `playground/src/lsp/session.rs:42`
 *     同一份固定表(见本文件 `TOKEN_TYPES`)。不在表里的索引视为「无颜色」跳过,
 *     不让一个未识别的 token 拉崩整段高亮。
 *   - `LspDiagnostic` 是 0 基 (line,column)..(end_line,end_column),按
 *     UTF-16 code unit 算(CodeMirror 默认也是 UTF-16,直接相加即可)。
 *   - `LspCompletion` 按 `request_id` 回显;前端 2 s 兜底 + 在途替换:发出去
 *     还没回就再发一条,旧 Promise 直接 resolve([]) —— 不排队,排队会和
 *     300 ms 内的下次按键撞上,弹窗会列错位的内容。
 *
 * 编辑器去抖 300 ms:早于此 RA 看到半行代码,晚于此人补全体感差
 * (Review Focus 第 1 条)。
 *
 * 高亮与诊断是两个独立的 decoration 层,可同时存在于同一 range
 * (Review Focus 第 5 条):高亮是 `Decoration.mark` 配 `cm-tok-*` class,
 * 诊断是 `Decoration.mark` 配 `cm-diag-*` class —— 两类 class 都同时
 * 挂在元素上,各自显示自己的颜色与下划线。
 */

import { Decoration, EditorView } from "https://esm.sh/@codemirror/view@6";
import { StateField, StateEffect } from "https://esm.sh/@codemirror/state@6";

/* LSP 标准 `tokenTypes` 的固定顺序。
 *
 * 必须与 `playground/src/lsp/session.rs:42` 同顺序 —— 服务端在
 * `initialize` 能力里把这个表报给 RA,RA 按这个顺序回 `tokenType` 索引;
 * 前端按同顺序把索引映射到 CSS class,顺序错一档整个高亮就乱。
 *
 * 不接收来自浏览器的 legend:服务端不向前端转发 `initialize` 响应
 * (Task 6 的协议),前端不依赖任何外部数据源 —— 协议层只透传 raw data。
 *
 * 而这里的下标**已经是我们的下标**:RA 按它自己那份 legend 编码,服务端
 * 在 `initialize` 之后按名字翻译过了(见 `playground/src/lsp/session.rs`
 * 的 `TokenLegend`)。表对不上时表现为整段颜色错位(比如关键字变斜体)。 */
const TOKEN_TYPES = [
    "namespace",      // 0
    "type",           // 1
    "class",          // 2
    "enum",           // 3
    "interface",      // 4
    "struct",         // 5
    "typeParameter",  // 6
    "parameter",      // 7
    "variable",       // 8
    "property",       // 9
    "enumMember",     // 10
    "event",          // 11
    "function",       // 12
    "method",         // 13
    "macro",          // 14
    "keyword",        // 15
    "modifier",       // 16
    "comment",        // 17
    "string",         // 18
    "number",         // 19
    "regexp",         // 20
    "operator",       // 21
    "decorator",      // 22
];

/* LSP `CompletionItemKind` → CodeMirror `Completion.type`。
 *
 * CodeMirror 的内置 type 是字符串表(见 codemirror/autocomplete 的命名);
 * 映射不到的降级到 "variable",不至于弹窗里没图标。 */
export function kindToType(kind) {
    if (kind == null) return "variable";
    switch (kind) {
        case 1: return "text";
        case 2: return "method";
        case 3: return "function";
        case 4: return "constructor";
        case 5: return "field";
        case 6: return "variable";
        case 7: return "class";
        case 8: return "interface";
        case 9: return "module";
        case 10: return "property";
        case 11: return "unit";
        case 12: return "value";
        case 13: return "enum";
        case 14: return "keyword";
        case 15: return "snippet";
        case 16: return "color";
        case 17: return "file";
        case 18: return "reference";
        case 19: return "folder";
        case 20: return "enum-member";
        case 21: return "constant";
        case 22: return "struct";
        case 23: return "event";
        case 24: return "operator";
        case 25: return "type-parameter";
        default: return "variable";
    }
}

/* ---------- State effects / fields ---------- */

export const setTokensEffect = StateEffect.define();
export const setDiagnosticsEffect = StateEffect.define();

/* 当前 token delta 数组(`LspTokens.data` 原样存)。`null` 表示 RA 还没给过 —
 * 创建时是 null,收到首帧 tokens 后切到数组。
 *
 * 持有原数组而不是解好的 range 列表,因为文档变化时 range 跟着平移,
 * 但 delta 是绝对坐标,跟着移动需要重新解一遍 —— 在 `buildDecorations` 里做。 */
export const tokensField = StateField.define({
    create() { return null; },
    update(value, tr) {
        for (const e of tr.effects) {
            if (e.is(setTokensEffect)) return e.value;
        }
        return value;
    },
});

/* 当前诊断列表(merge 后的最终状态,服务端按 LSP 语义一次给完整集合;
 * connection-level 上 LspDiagnostic 一条一条来,这里按 range key 去重累加)。 */
export const diagnosticsField = StateField.define({
    create() { return []; },
    update(value, tr) {
        for (const e of tr.effects) {
            if (e.is(setDiagnosticsEffect)) return e.value;
        }
        return value;
    },
});

/* 把 RA delta 数组解成一组 (from, to, typeIndex) —— typeIndex 是 TOKEN_TYPES
 * 里的索引,不在表里的丢弃(防御性:legend 漂移 / 旧数据时不炸)。
 *
 * delta 编码:LSP 用相对前一个 token 的位移来压缩。deltaLine > 0 时 startChar
 * 是新一行的绝对字符偏移,否则是相对前一个 token 的字符偏移。 */
function decodeTokens(doc, data) {
    const out = [];
    if (!data || data.length === 0) return out;
    let line = 0;
    let startChar = 0;
    for (let i = 0; i + 5 <= data.length; i += 5) {
        const dLine = data[i];
        const dStart = data[i + 1];
        const length = data[i + 2];
        const tokenType = data[i + 3];
        // data[i + 4] 是 tokenModifiers,前端不染 modifier —— 留给后续按需扩展。
        if (dLine > 0) {
            line += dLine;
            startChar = dStart;
        } else {
            startChar += dStart;
        }
        if (tokenType < 0 || tokenType >= TOKEN_TYPES.length) continue;
        // CodeMirror 行号是 1 基,LSP 是 0 基;这里用 `line + 1`。
        // 越界(line 超过 doc 行数)说明 RA 给的 token 是在它重发过程中文档又变了,
        // 丢掉这一项而不是让 editor 抛 RangeError。
        if (line < 0 || line >= doc.lines) continue;
        const lineObj = doc.line(line + 1);
        const from = lineObj.from + startChar;
        const to = from + length;
        // `from >= to` 都丢:CM 的 Mark decoration **不允许空 range**,抛
        // `RangeError: Mark decorations may not be empty` —— 一抛整个
        // decoration 字段的事务都失败,后面的高亮/诊断全不显示。
        if (from < 0 || to > doc.length || from >= to) continue;
        out.push({ from, to, type: tokenType });
    }
    return out;
}

/* 诊断 → CodeMirror range。同上,越界丢。 */
function diagnosticRanges(doc, diags) {
    const out = [];
    for (const d of diags) {
        if (d.line < 0 || d.line >= doc.lines) continue;
        if (d.end_line < 0 || d.end_line >= doc.lines) continue;
        const fromLine = doc.line(d.line + 1);
        const toLine = doc.line(d.end_line + 1);
        let from = fromLine.from + d.column;
        let to = toLine.from + d.end_column;
        if (from < 0 || to > doc.length) continue;
        // RA 的零长诊断(典型:`unexpected end of input`,位置在文件末尾)
        // 会让 `Decoration.mark().range(x, x)` 抛 RangeError,整个 decoration
        // 事务失败。往可用的方向借一个字符,让波浪线至少有地方画;文档为空
        // 时借不到,只能丢。
        if (from === to) {
            if (to < doc.length) to += 1;
            else if (from > 0) from -= 1;
            else continue;
        }
        out.push({ from, to, severity: d.severity });
    }
    return out;
}

function severityClass(sev) {
    if (sev === 1) return "cm-diag-error";
    if (sev === 2) return "cm-diag-warning";
    // LSP 0 在规范里是「未使用值」,服务端允许把它当 Information 处理
    // (playground/src/protocol.rs:127 的注释)。
    if (sev === 3 || sev === 0) return "cm-diag-info";
    if (sev === 4) return "cm-diag-hint";
    return null;
}

/* 单个 decoration 字段:高亮(`cm-tok-*`)和诊断(`cm-diag-*`)两层同时叠加。
 * 是两个独立的 DecorationSet 子树,各自只染对应 class —— 同一 range 可同时
 * 被两层覆盖,各自不抢颜色。 */
function buildDecorations(state) {
    const tokens = state.field(tokensField, false);
    const diags = state.field(diagnosticsField, false);
    const doc = state.doc;
    const out = [];
    if (tokens) {
        const decoded = decodeTokens(doc, tokens);
        for (const t of decoded) {
            out.push(
                Decoration.mark({ class: "cm-tok-" + TOKEN_TYPES[t.type] })
                    .range(t.from, t.to)
            );
        }
    }
    if (diags) {
        const decoded = diagnosticRanges(doc, diags);
        for (const d of decoded) {
            const cls = severityClass(d.severity);
            if (!cls) continue;
            out.push(Decoration.mark({ class: cls }).range(d.from, d.to));
        }
    }
    return out;
}

/* 暴露给 `EditorView.decorations.from`。文档变化时 `map(changes)` 让 decorations
 * 跟着移动 —— 字符插入 / 删除不会让整段高亮飞掉。 */
export const decorationField = StateField.define({
    create() { return Decoration.none; },
    update(deco, tr) {
        if (tr.effects.some((e) => e.is(setTokensEffect) || e.is(setDiagnosticsEffect))) {
            return Decoration.set(buildDecorations(tr.state));
        }
        if (tr.docChanged) return deco.map(tr.changes);
        return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
});

const DEBOUNCE_MS = 300;
const COMPLETION_TIMEOUT_MS = 2000;

/* `LspClient` 只做两件事:
 *   - 收 `LspTokens` / `LspDiagnostic` / `LspCompletion`,dispatch 给 editor
 *   - editor 文档变化时去抖 300 ms,再发 `LspChange`
 *
 * `LspCompletion` 的请求-应答靠 `request_id` 配对 + 2 s 兜底 timeout:
 * 超过 2 s 就当 RA 没响应,resolve([]),CodeMirror 的弹窗不卡死。
 *
 * 在途替换:新请求发出时旧 Promise 直接 resolve([]),不排队 —— 排队会和
 * 300 ms 内的下次按键撞上,弹窗里显示错位的内容。 */
export class LspClient {
    /**
     * @param {object} opts
     * @param {import('@codemirror/view').EditorView} opts.view
     * @param {(msg: object) => void} opts.send 把消息发到 ws(已经 JSON 序列化好;
     *     这里只关心能不能送到,序列化是 ws.send 的活)。
     * @param {(text: string) => void} [opts.onStatus] UI 状态更新;
     *     在 lsp_ready 时被调用一次。
     */
    constructor(opts) {
        this.view = opts.view;
        this.send = opts.send;
        this.onStatus = opts.onStatus || (() => {});
        this._ready = false;
        this._debounceTimer = null;
        this._pendingSource = null;
        this._nextRequestId = 1;
        this._pendingCompletions = new Map();
        // 同一帧里攒着的诊断与「要不要刷」的标记,见 _scheduleDiagFlush。
        this._pendingDiag = [];
        this._diagFlushScheduled = false;
    }

    /** 编辑器建好之后调用一次,会发 `LspOpen`。 */
    open() {
        this.send({ type: "lsp_open" });
    }

    /** `LspReady` 到达。状态切到就绪,并把当前文档内容同步给 RA ——
     * 占位内容是服务端在补池时 did_open 进去的,RA 看到的是
     * `// playground: waiting for user source`。需要把用户的实际源码
     * (可能为空 / 可能是已载入的样例)一次性发过去,RA 才会出第一帧 tokens / 诊断。 */
    onReady() {
        this._ready = true;
        this.onStatus("已就绪 · 分析在线");
        if (this.view) {
            // 不走去抖:这时编辑器的当前内容和服务端占位内容是两份,
            // 必须把整份立即推给 RA,否则 RA 重算的 semanticTokens 仍基于占位内容。
            this.send({ type: "lsp_change", source: this.view.state.doc.toString() });
        }
    }

    /** 一条 server 消息(`type` 已经剥好)。 */
    onMessage(msg) {
        switch (msg.type) {
            case "lsp_ready":
                this.onReady();
                break;
            case "lsp_tokens":
                this._applyTokens(msg.data || []);
                break;
            case "lsp_diagnostic":
                // **攒一帧再 dispatch。** 服务端一条诊断一条消息发,RA 改一行
                // 常给五条 —— 直接逐条 dispatch 就是五次 CM 事务、五个中间
                // 状态(波浪线先亮三条再齐)。rAF 把同一帧里的消息并成一次。
                this._pendingDiag.push(msg);
                this._scheduleDiagFlush();
                break;
            case "lsp_completion":
                this._resolveCompletion(msg.request_id, msg.items || []);
                break;
        }
    }

    /** 编辑器的文档变化。外部(主线程)直接调。 */
    onDocChanged(source) {
        this._pendingSource = source;
        if (this._debounceTimer !== null) clearTimeout(this._debounceTimer);
        this._debounceTimer = setTimeout(() => {
            this._debounceTimer = null;
            const s = this._pendingSource;
            this._pendingSource = null;
            // 没 ready 就丢 —— 服务端 LspChange 在未 open 时回 BadRequest。
            // onReady 会重新同步当前文档,这里静默丢没问题。
            if (!this._ready) return;
            this.send({ type: "lsp_change", source: s });
        }, DEBOUNCE_MS);
    }

    /** 用户触发补全。返回 Promise<items>。
     *
     * 在途替换:已有挂起的直接 resolve([]) 并清掉 timer。2 s 内 RA 没回
     * 就 resolve([]),不让弹窗空转。LSP 未就绪时直接返回空数组 —— 服务端
     * 收到未 open 的 LspCompletion 会回 BadRequest,前端没理由踩这条。 */
    requestCompletion(line, column) {
        if (!this._ready) return Promise.resolve([]);
        for (const [, entry] of this._pendingCompletions) {
            clearTimeout(entry.timer);
            entry.resolve([]);
        }
        this._pendingCompletions.clear();
        const id = this._nextRequestId++;
        return new Promise((resolve) => {
            const timer = setTimeout(() => {
                this._pendingCompletions.delete(id);
                resolve([]);
            }, COMPLETION_TIMEOUT_MS);
            this._pendingCompletions.set(id, { resolve, timer });
            this.send({ type: "lsp_completion", request_id: id, line, column });
        });
    }

    /** 销毁:清掉 timer 与挂起的补全 Promise。
     *
     * **连接重连时必须调。** 否则旧实例挂着的去抖 timer 会在新实例已经
     * `lsp_open` 之后触发,拿旧编辑框的内容发一条 `lsp_change` 过去 ——
     * 一次多余的 RA 往返,以及一个用户没见过的中间状态。 */
    dispose() {
        if (this._debounceTimer !== null) {
            clearTimeout(this._debounceTimer);
            this._debounceTimer = null;
        }
        this._pendingSource = null;
        // 攒着的诊断不再有意义:新连接会重新推一遍文档,RA 会重发。
        this._pendingDiag = [];
        this._diagFlushScheduled = false;
        for (const [, entry] of this._pendingCompletions) {
            clearTimeout(entry.timer);
            entry.resolve([]);
        }
        this._pendingCompletions.clear();
    }

    _applyTokens(data) {
        if (!this.view) return;
        this.view.dispatch({ effects: setTokensEffect.of(data) });
    }

    _scheduleDiagFlush() {
        if (this._diagFlushScheduled) return;
        this._diagFlushScheduled = true;
        requestAnimationFrame(() => {
            this._diagFlushScheduled = false;
            const list = this._pendingDiag;
            this._pendingDiag = [];
            this._applyDiagnostics(list);
        });
    }

    _applyDiagnostics(list) {
        if (!this.view || list.length === 0) return;
        // 累积:服务端按 LSP 语义一次给完整集合,这里按 (line,column,end_line,end_column)
        // 去重 + 后到的覆盖同 range。`LspDiagnostic` 一次只来一条但连接上的同一帧可能多条。
        const map = new Map();
        const cur = this.view.state.field(diagnosticsField, false) || [];
        for (const d of cur) {
            map.set(diagKey(d), d);
        }
        for (const m of list) {
            map.set(diagKey(m), {
                line: m.line,
                column: m.column,
                end_line: m.end_line,
                end_column: m.end_column,
                severity: m.severity,
                message: m.message,
            });
        }
        const merged = [...map.values()];
        this.view.dispatch({ effects: setDiagnosticsEffect.of(merged) });
    }

    _resolveCompletion(request_id, items) {
        const entry = this._pendingCompletions.get(request_id);
        if (!entry) return;
        clearTimeout(entry.timer);
        this._pendingCompletions.delete(request_id);
        entry.resolve(items);
    }
}

function diagKey(d) {
    return `${d.line}:${d.column}:${d.end_line}:${d.end_column}`;
}