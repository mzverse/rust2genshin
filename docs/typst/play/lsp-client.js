/* lsp-client.js —— 编辑器和 RA 之间的最小 JSON-RPC 层。
 *
 * 服务端只做会话与管道：除 initialize / initialized / shutdown / exit /
 * didOpen / didChange 外，消息一律原样双向转发（见
 * docs/superpowers/specs/2026-10-19-play-lsp-raw-proxy-design.md）。
 * 所以这里自己发请求、自己消费结果：
 *   - semantic tokens：用 initialize 响应里的真实 legend 把索引翻成名字，
 *     再映射到 `cm-tok-*`；名字不在支持列表里的 token 跳过。
 *   - 诊断：拉取式 `textDocument/diagnostic`（`full` 整份替换、
 *     `unchanged` 保留上一份），同时兼容推送式 `publishDiagnostics`。
 *   - 补全：300 ms 去抖 + 2 s 兜底 + 在途替换。
 *
 * 高亮与诊断是两个独立的 decoration 层，可同时存在于同一 range。
 */

import { Decoration, EditorView } from "https://esm.sh/@codemirror/view@6";
import { StateField, StateEffect } from "https://esm.sh/@codemirror/state@6";

/* 我们真的有样式的 token 名。`style.css` 里每个名字都有一条
 * `.cm-tok-<name>`；不在表里的名字没有 class，直接跳过。 */
const SUPPORTED_TOKENS = new Set([
    "namespace", "type", "class", "enum", "interface", "struct",
    "typeParameter", "parameter", "variable", "property", "enumMember",
    "event", "function", "method", "macro", "keyword", "modifier",
    "comment", "string", "number", "regexp", "operator", "decorator",
]);

/* RA 没有上报 legend 时的兜底顺序（LSP 标准顺序）。RA 1.10x 总会报，
 * 这里只是防御：没有 legend 就什么都画不出来更糟。 */
const FALLBACK_TOKEN_TYPES = [...SUPPORTED_TOKENS];

/* LSP `CompletionItemKind` → CodeMirror `Completion.type`。
 *
 * CodeMirror 的内置 type 是字符串表（见 codemirror/autocomplete 的命名）；
 * 映射不到的降级到 "variable"，不至于弹窗里没图标。 */
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

/* 当前 token 数据：`{ data, types }`。`types` 是 initialize 响应里的
 * legend，解码时逐项查名字。`null` 表示还没收到过。 */
export const tokensField = StateField.define({
    create() { return null; },
    update(value, tr) {
        for (const e of tr.effects) {
            if (e.is(setTokensEffect)) return e.value;
        }
        return value;
    },
});

/* 当前诊断列表（整份）。空数组就是清空。 */
export const diagnosticsField = StateField.define({
    create() { return []; },
    update(value, tr) {
        for (const e of tr.effects) {
            if (e.is(setDiagnosticsEffect)) return e.value;
        }
        return value;
    },
});

/* delta 数组是 5 个一组 `(Δline, Δstart, len, type, modifiers)`。
 * 解码时把 type 索引查成名字；名字不在 SUPPORTED_TOKENS 里就跳过。 */
function decodeTokens(doc, data, types) {
    const out = [];
    if (!data || data.length === 0) return out;
    let line = 0;
    let startChar = 0;
    for (let i = 0; i + 5 <= data.length; i += 5) {
        const dLine = data[i];
        const dStart = data[i + 1];
        const length = data[i + 2];
        const typeIndex = data[i + 3];
        if (dLine > 0) {
            line += dLine;
            startChar = dStart;
        } else {
            startChar += dStart;
        }
        const name = types[typeIndex];
        if (!name || !SUPPORTED_TOKENS.has(name)) continue;
        if (line < 0 || line >= doc.lines) continue;
        const lineObj = doc.line(line + 1);
        const from = lineObj.from + startChar;
        const to = from + length;
        // 空 range 会让 `Decoration.mark` 抛 RangeError，整批装饰失败。
        if (from < 0 || to > doc.length || from >= to) continue;
        out.push({ from, to, name });
    }
    return out;
}

/* LSP `Diagnostic`（原始形状）→ CodeMirror range。越界丢。 */
function diagnosticRanges(doc, diags) {
    const out = [];
    for (const d of diags) {
        const range = d && d.range;
        if (!range || !range.start || !range.end) continue;
        const line = range.start.line;
        const endLine = range.end.line;
        if (line < 0 || line >= doc.lines) continue;
        if (endLine < 0 || endLine >= doc.lines) continue;
        const fromLine = doc.line(line + 1);
        const toLine = doc.line(endLine + 1);
        let from = fromLine.from + range.start.character;
        let to = toLine.from + range.end.character;
        if (from < 0 || to > doc.length) continue;
        // RA 的零长诊断会让 `Decoration.mark().range(x, x)` 抛 RangeError；
        // 往可用的方向借一个字符。
        if (from === to) {
            if (to < doc.length) to += 1;
            else if (from > 0) from -= 1;
            else continue;
        }
        out.push({ from, to, severity: d.severity ?? 1, message: d.message || "" });
    }
    return out;
}

function severityClass(sev) {
    if (sev === 1) return "cm-diag-error";
    if (sev === 2) return "cm-diag-warning";
    if (sev === 3 || sev === 0) return "cm-diag-info";
    if (sev === 4) return "cm-diag-hint";
    return null;
}

/* 高亮与诊断两层同时叠加。 */
function buildDecorations(state) {
    const tokens = state.field(tokensField, false);
    const diags = state.field(diagnosticsField, false);
    const doc = state.doc;
    const out = [];
    if (tokens) {
        for (const t of decodeTokens(doc, tokens.data, tokens.types)) {
            out.push(
                Decoration.mark({ class: "cm-tok-" + t.name }).range(t.from, t.to)
            );
        }
    }
    if (diags) {
        for (const d of diagnosticRanges(doc, diags)) {
            const cls = severityClass(d.severity);
            if (!cls) continue;
            const spec = { class: cls };
            if (d.message) spec.attributes = { title: d.message };
            out.push(Decoration.mark(spec).range(d.from, d.to));
        }
    }
    return out;
}

export const decorationField = StateField.define({
    create() { return Decoration.none; },
    update(deco, tr) {
        if (tr.effects.some((e) => e.is(setTokensEffect) || e.is(setDiagnosticsEffect))) {
            // 第二个参数必须为 true：两层拼起来不一定有序。
            return Decoration.set(buildDecorations(tr.state), true);
        }
        if (tr.docChanged) return deco.map(tr.changes);
        return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
});

const DEBOUNCE_MS = 300;
const COMPLETION_TIMEOUT_MS = 2000;

/* 把 RA 的补全响应当成两种形状收：裸数组或 `{ items }`；字段名
 * `labelDetails` 归一成 `label_details`，与 `script.js` 的渲染保持一致。 */
function normalizeCompletion(result) {
    const items = Array.isArray(result)
        ? result
        : (result && Array.isArray(result.items) ? result.items : []);
    return items
        .filter((item) => item && typeof item.label === "string")
        .map((item) => ({
            label: item.label,
            kind: item.kind ?? null,
            detail: item.detail ?? null,
            label_details: item.labelDetails
                ? {
                    detail: item.labelDetails.detail ?? null,
                    description: item.labelDetails.description ?? null,
                }
                : null,
        }));
}

export class LspClient {
    /**
     * @param {object} opts
     * @param {import('@codemirror/view').EditorView} opts.view
     * @param {(msg: object) => void} opts.send
     * @param {(text: string) => void} [opts.onStatus]
     */
    constructor(opts) {
        this.view = opts.view;
        this.send = opts.send;
        this.onStatus = opts.onStatus || (() => {});
        this.uri = null;
        this._ready = false;
        this._disposed = false;
        this._nextId = 1;
        this._version = 0;
        this._tokenTypes = FALLBACK_TOKEN_TYPES;
        this._pending = new Map();
        this._completionIds = new Set();
        this._debounceTimer = null;
        this._pendingSource = null;
    }

    /** 编辑器建好之后调用一次，会发 `lsp_open`。 */
    open() {
        this.send({ type: "lsp_open" });
    }

    /** `lsp_ready` 到达：开始握手。 */
    onReady(uri) {
        this.uri = uri;
        this.onStatus("分析中…");
        this._initialize();
    }

    async _initialize() {
        let result;
        try {
            result = await this._request("initialize", {
                // 服务端不转发这条；capabilities 以服务端待命时报的那份为准。
                processId: null,
                rootUri: null,
                capabilities: {},
            });
        } catch {
            return; // 断线 / 合成的 exit：状态由 onClosed 统一收。
        }
        if (this._disposed || !this.view) return;

        const legend = result &&
            result.capabilities &&
            result.capabilities.semanticTokensProvider &&
            result.capabilities.semanticTokensProvider.legend;
        const names = legend && legend.tokenTypes;
        this._tokenTypes = Array.isArray(names) && names.length
            ? names
            : FALLBACK_TOKEN_TYPES;

        this._notify("initialized", {});
        this._ready = true;
        this.onStatus("已就绪 · 分析在线");
        this._version = 1;
        this._notify("textDocument/didOpen", {
            textDocument: {
                uri: this.uri,
                languageId: "rust",
                version: 1,
                text: this.view.state.doc.toString(),
            },
        });
        this._refreshTokens();
        this._refreshDiagnostics();
    }

    /** 一条 RA 的 LSP 报文。 */
    handleMessage(message) {
        if (!message || typeof message !== "object") return;
        if (typeof message.method === "string") {
            if (message.method === "exit") {
                // 服务端合成的关闭信号（规范里 exit 是客户端→服务端，
                // 这里是本项目唯一的反向帧）。
                this.onClosed((message.params && message.params.reason) || "分析器已退出");
                return;
            }
            if (message.id !== undefined && message.id !== null) {
                // RA 的请求：统一回 null，漏答会让它挂起。
                this._sendMessage({ jsonrpc: "2.0", id: message.id, result: null });
                if (message.method === "workspace/semanticTokens/refresh") {
                    this._refreshTokens();
                } else if (message.method === "workspace/diagnostic/refresh") {
                    this._refreshDiagnostics();
                }
                return;
            }
            if (message.method === "textDocument/publishDiagnostics") {
                this._publishDiagnostics(message.params);
            }
            return;
        }
        const entry = this._pending.get(message.id);
        if (!entry) return;
        this._pending.delete(message.id);
        if (entry.timer !== null) clearTimeout(entry.timer);
        if (message.error) {
            entry.reject(new Error(message.error.message || "LSP error"));
        } else {
            entry.resolve(message.result);
        }
    }

    /** 收到合成的 `exit`：清高亮/诊断，挂起的请求全部收掉。 */
    onClosed(message) {
        this._ready = false;
        for (const [, entry] of this._pending) {
            if (entry.timer !== null) clearTimeout(entry.timer);
            entry.reject(new Error(message || "LSP closed"));
        }
        this._pending.clear();
        this._completionIds.clear();
        if (this.view) {
            this.view.dispatch({ effects: setTokensEffect.of(null) });
            this.view.dispatch({ effects: setDiagnosticsEffect.of([]) });
        }
        this.onStatus("代码分析离线");
    }

    /** 编辑器的文档变化。外部（主线程）直接调。 */
    onDocChanged(source) {
        this._pendingSource = source;
        if (this._debounceTimer !== null) clearTimeout(this._debounceTimer);
        this._debounceTimer = setTimeout(() => {
            this._debounceTimer = null;
            this._flushChange();
        }, DEBOUNCE_MS);
    }

    /** 立刻推掉还没发的改动：补全请求不能跑在旧文上。 */
    _flushChange() {
        if (this._debounceTimer !== null) {
            clearTimeout(this._debounceTimer);
            this._debounceTimer = null;
        }
        const source = this._pendingSource;
        this._pendingSource = null;
        if (source === null || !this._ready || !this.view) return;
        const version = ++this._version;
        this._notify("textDocument/didChange", {
            textDocument: { uri: this.uri, version },
            contentChanges: [{ text: source }],
        });
        this._refreshTokens();
        this._refreshDiagnostics();
    }

    /** 用户触发补全。返回 Promise<items>。 */
    requestCompletion(line, column) {
        if (!this._ready) return Promise.resolve([]);
        this._flushChange();
        // 在途替换：旧请求直接 resolve(null)，不排队。
        for (const id of this._completionIds) {
            const entry = this._pending.get(id);
            if (!entry) continue;
            if (entry.timer !== null) clearTimeout(entry.timer);
            this._pending.delete(id);
            entry.resolve(null);
        }
        this._completionIds.clear();

        const id = this._nextId++;
        return new Promise((resolve) => {
            const timer = setTimeout(() => {
                this._pending.delete(id);
                this._completionIds.delete(id);
                resolve([]);
            }, COMPLETION_TIMEOUT_MS);
            this._pending.set(id, {
                resolve: (result) => resolve(normalizeCompletion(result)),
                reject: () => resolve([]),
                timer,
            });
            this._completionIds.add(id);
            this._sendMessage({
                jsonrpc: "2.0",
                id,
                method: "textDocument/completion",
                params: {
                    textDocument: { uri: this.uri },
                    position: { line, character: column },
                },
            });
        });
    }

    /** 销毁：连接重连时必须调，清掉 timer 与挂起请求。 */
    dispose() {
        this._disposed = true;
        this._ready = false;
        if (this._debounceTimer !== null) {
            clearTimeout(this._debounceTimer);
            this._debounceTimer = null;
        }
        this._pendingSource = null;
        for (const [, entry] of this._pending) {
            if (entry.timer !== null) clearTimeout(entry.timer);
            entry.reject(new Error("LSP client disposed"));
        }
        this._pending.clear();
        this._completionIds.clear();
    }

    _sendMessage(message) {
        this.send({ type: "lsp", message });
    }

    _notify(method, params) {
        this._sendMessage({ jsonrpc: "2.0", method, params });
    }

    _request(method, params) {
        const id = this._nextId++;
        return new Promise((resolve, reject) => {
            this._pending.set(id, { resolve, reject, timer: null });
            this._sendMessage({ jsonrpc: "2.0", id, method, params });
        });
    }

    _refreshTokens() {
        if (!this._ready) return;
        this._request("textDocument/semanticTokens/full", {
            textDocument: { uri: this.uri },
        }).then((result) => {
            if (!this._ready || !this.view || !result) return;
            const data = Array.isArray(result.data) ? result.data : [];
            this.view.dispatch({
                effects: setTokensEffect.of({ data, types: this._tokenTypes }),
            });
        }).catch(() => {});
    }

    _refreshDiagnostics() {
        if (!this._ready) return;
        this._request("textDocument/diagnostic", {
            textDocument: { uri: this.uri },
        }).then((result) => {
            if (!this._ready || !result) return;
            // `unchanged` 不带 items：保留现有那一份。
            if (result.kind === "unchanged") return;
            const items = Array.isArray(result) ? result : result.items;
            if (Array.isArray(items)) this._setDiagnostics(items);
        }).catch(() => {});
    }

    _publishDiagnostics(params) {
        if (!this.view || !params) return;
        if (params.uri !== this.uri) return;
        this._setDiagnostics(params.diagnostics || []);
    }

    _setDiagnostics(list) {
        if (!this.view) return;
        this.view.dispatch({ effects: setDiagnosticsEffect.of(list) });
    }
}