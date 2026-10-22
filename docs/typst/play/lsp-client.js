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

import { Decoration, EditorView, hoverTooltip } from "https://esm.sh/@codemirror/view@6";
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
            // 消息放在 aria-label 里,不改用 title:title 会招来浏览器原生
            // 气泡,和下面的 hoverTooltip 叠成两层。原生气泡是浏览器 UI,
            // 既不进页面渲染树(截屏录不到),样式也不受控。
            if (d.message) spec.attributes = { "aria-label": d.message };
            out.push(Decoration.mark(spec).range(d.from, d.to));
        }
    }
    return out;
}

/* 诊断的悬停提示。
 *
 * `buildDecorations` 只负责画波浪线;消息本身用 CM 的 hoverTooltip 显示 ——
 * 提示是页面里的 DOM 节点,和正文一起渲染,截屏/录屏都看得到,样式也跟主题
 * 一致。悬停 250ms 后出现,移开即消失。 */
export const diagnosticTooltip = hoverTooltip((view, pos) => {
    const diags = view.state.field(diagnosticsField, false);
    if (!diags) return null;
    for (const d of diagnosticRanges(view.state.doc, diags)) {
        if (pos < d.from || pos > d.to || !d.message) continue;
        return {
            pos: d.from,
            end: d.to,
            above: true,
            create() {
                const dom = document.createElement("div");
                dom.className = "cm-diag-tip";
                if (d.severity === 2) dom.classList.add("is-warning");
                else if (d.severity !== 1) dom.classList.add("is-info");
                dom.textContent = d.message;
                return { dom };
            },
        };
    }
    return null;
}, { hoverTime: 250 });

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

/* 源码体量上限 —— **字节**,不是字符数。服务端按 `str::len()`(UTF-8
 * 字节)比,前端就得用 TextEncoder 算,两边才对得上。用
 * `String.length` 的话一个中文算 1,跟服务端差三倍。
 *
 * 上限是服务端在 `Hello.lsp_max_source_bytes` 里给的,前端不写死:
 * 运维改阈值不用重编前端。没拿到(旧后端)就是不限。 */
function byteLength(text) {
    return new TextEncoder().encode(text).length;
}

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
     * @param {number} [opts.maxBytes] 源码体量上限(字节)。缺省 = 不限。
     */
    constructor(opts) {
        this.view = opts.view;
        this.send = opts.send;
        this.onStatus = opts.onStatus || (() => {});
        this.maxBytes = typeof opts.maxBytes === "number" && isFinite(opts.maxBytes)
            ? opts.maxBytes
            : Infinity;
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
        // 是否已经发出过 `lsp_open`。超限时**不发** —— 服务端那边会
        // 起一个 RA 容器(常驻 839 MB),给一份根本不会分析的源码起它
        // 是纯浪费。
        this._opened = false;
        // RA 那边是不是已经有这份文档了(`didOpen` 发过没有)。超限时
        // 不发 `didOpen`,那 RA 手上一份文档都没有 —— 恢复时必须补
        // `didOpen`,发 `didChange` 是非法的。
        this._docOpen = false;
        // 当前源码是不是超限。
        this._overCap = false;
    }

    /* 源码是否超过上限。 */
    _isOverCap(text) {
        return byteLength(text) > this.maxBytes;
    }

    /* 超限时的提示。措辞与服务端那条一致(`ws.rs` 的 `lsp_cap_message`),
     * 两边说出来的话像同一件事。 */
    _reportOverCap(text) {
        this.onStatus(
            `源码 ${byteLength(text)} 字节,超过 ${this.maxBytes} 字节上限,代码分析已停用`);
        // 高亮和诊断都是旧源码的结论 —— 留着比没有更糟。
        this._clearDecorations();
    }

    _clearDecorations() {
        if (!this.view) return;
        this.view.dispatch({ effects: setTokensEffect.of(null) });
        this.view.dispatch({ effects: setDiagnosticsEffect.of([]) });
    }

    /** 编辑器建好之后调用一次，会发 `lsp_open`。 */
    open() {
        const text = this.view ? this.view.state.doc.toString() : "";
        if (this._isOverCap(text)) {
            // 超限:不开 RA 会话。等用户把代码删回上限以下,再补开。
            this._overCap = true;
            this._opened = false;
            this._reportOverCap(text);
            return false;
        }
        this._overCap = false;
        this._opened = true;
        this.send({ type: "lsp_open" });
        return true;
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
        const text = this.view.state.doc.toString();
        // 握手这段时间里源码可能已经变大了 —— 再量一次,超限就别送
        // `didOpen`。RA 那边开着会话但没有文档,不会白烧 CPU。
        if (this._isOverCap(text)) {
            this._overCap = true;
            this._docOpen = false;
            this._reportOverCap(text);
            return;
        }
        this._docOpen = true;
        this._notify("textDocument/didOpen", {
            textDocument: {
                uri: this.uri,
                languageId: "rust",
                version: 1,
                text,
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
        if (source === null || !this.view) return;

        // 超限 → 停发。RA 那边留着旧文档继续分析,高亮和诊断清掉 ——
        // 那些结论是旧源码的,留着比没有更糟。
        if (this._isOverCap(source)) {
            if (!this._overCap) {
                this._overCap = true;
                this._reportOverCap(source);
            }
            return;
        }
        // 回落 → 恢复。超限期间根本没开过会话就现在开;开过的话把
        // 全文重新推一遍(整份替换),让 RA 跟上来。
        if (this._overCap) {
            this._overCap = false;
            this.onStatus("代码分析已恢复");
            if (!this._opened) {
                this._opened = true;
                this.send({ type: "lsp_open" });
                return;
            }
            if (!this._ready) return;
            if (!this._docOpen) {
                // RA 手上还没有这份文档(握手时就超限):补一次 `didOpen`。
                // 只发 `didChange` 的话 RA 那边没有对应的文档,报文是
                // 非法的。
                this._docOpen = true;
                this._version = 1;
                this._notify("textDocument/didOpen", {
                    textDocument: {
                        uri: this.uri,
                        languageId: "rust",
                        version: 1,
                        text: source,
                    },
                });
            } else {
                this._version++;
                this._notify("textDocument/didChange", {
                    textDocument: { uri: this.uri, version: this._version },
                    contentChanges: [{ text: source }],
                });
            }
            this._refreshTokens();
            this._refreshDiagnostics();
            return;
        }

        if (!this._ready || !this._opened) return;
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
        // 超限期间 RA 手上是旧文档,补全会给出过时的候选 —— 不如不给。
        if (!this._ready || this._overCap) return Promise.resolve([]);
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