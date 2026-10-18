/* 摆弄页。
 *
 * 编辑器从 `<mdui-text-field textarea>` 换成了 CodeMirror 6;语义高亮 / 诊断 /
 * 补全都由 RA 经后端 WebSocket 给,本地零解析 —— 不存在「高亮和分析结论
 * 不一致」(Review Focus 第 5 条)。
 *
 * 与 `lsp-client.js` 的分工:
 *   - script.js 管「WS 连上 / 发 submit / 收消息」+ UI 状态(target 选择器、
 *     日志面板、拖动条、选项卡、样例载入);
 *   - lsp-client.js 管「LspTokens / LspDiagnostic / LspCompletion → CodeMirror
 *     编辑器 → LspChange / LspCompletion」的具体实现(去抖 300 ms、2 s 兜底、
 *     在途替换)。CodeMirror 的 effects / fields 也定义在那儿,这一个文件
 *     集中 LSP 相关的内容。
 *
 * 后端地址不归页面管 —— 见下面 `BACKENDS` 的注释。
 */

import { EditorView, keymap, lineNumbers, highlightActiveLine } from "https://esm.sh/@codemirror/view@6";
import { EditorState, Compartment } from "https://esm.sh/@codemirror/state@6";
import { history, historyKeymap } from "https://esm.sh/@codemirror/commands@6";
import { autocompletion, closeBrackets } from "https://esm.sh/@codemirror/autocomplete@6";
import { indentation, indentOnInput, indentUnit } from "https://esm.sh/@codemirror/language@6";
import { rust } from "https://esm.sh/@codemirror/lang-rust@6";
import {
    LspClient,
    kindToType,
    tokensField,
    diagnosticsField,
    decorationField,
} from "./lsp-client.js";

/* 后端地址表。**页面里不给看也不给改** —— 写死在这儿,原因有三个:
 *   - 地址一改就要重新构建页面,不是访客该管的事;
 *   - 之前那个 localStorage 输入框让人能把自己页面指到任意 wss:// 上,
 *     编译日志和 .gia 都从那儿取,等于开了一条任意地址的抓取通道;
 *   - 多后端轮询要的是「一组」地址,单个可编辑框表达不了。
 *
 * 打乱 + 挨个试是为了把负载摊开:谁都优先连自己列表里的第一个,列表尾部
 * 的后端会一直没人用。 */
const BACKENDS = [
    "wss://www.mzverse.org:6/rust2genshin/ws",
    "ws://127.0.0.1:8787/ws",
];

/* target 真值 → 显示名。前端负责把 `Hello.targets` 里那一串 `beyond-server` /
 * `classic-server` 翻成「超限模式」/「经典模式」。这张表只在前端维护
 * (后端不在意谁是什么模式),不在表里的目标降级到原值 —— 加新目标时改这里一行。
 *
 * 顺序由 `Hello.targets` 决定(`protocol::TARGETS`,协议侧唯一真源),
 * 前端不写死顺序。 */
const TARGET_LABELS = {
    "beyond-server": "超限模式",
    "classic-server": "经典模式",
};

/* Fisher–Yates。每次加载都重新打乱,所以刷新页面就是在换一条起点。 */
function shuffled(list) {
    const a = list.slice();
    for (let i = a.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
}

/* 挨个地址试,连上就 resolve。**不管排不排队** —— 排上队的后端也是活的后端,
 * 为此丢掉一个排队位不划算。
 * 一个都没连上就 reject,原因取最后一个(也就是最接近能用)的那个。 */
function pickBackend(urls) {
    let last = new Error("没有配后端");
    return urls
        .reduce(
            (chain, url) =>
                chain.then((hit) => hit || tryConnect(url).catch((e) => {
                    last = e;
                    return null;
                })),
            Promise.resolve(null)
        )
        .then((hit) => {
            if (!hit) throw last;
            return hit;
        });
}

/* WebSocket 的 open/error/close 都是异步的,这里包一层「只认第一次结果」的
 * 壳,第一个 settle 赢。握手本身没有超时机制,光等 ws 自己报错要几十秒
 * (TCP 层面),所以另起一个定时器主动收摊。 */
function tryConnect(url, timeoutMs = 4000) {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(url);
        let done = false;
        const finish = (fn, arg) => {
            if (done) return;
            done = true;
            clearTimeout(timer);
            fn(arg);
        };
        const timer = setTimeout(() => {
            finish(reject, new Error("连接超时"));
            try {
                ws.close();
            } catch {
                /* 已经断了,close 抛了也没关系 */
            }
        }, timeoutMs);
        ws.onopen = () => finish(resolve, ws);
        ws.onerror = () => finish(reject, new Error("连不上"));
        ws.onclose = () => finish(reject, new Error("连接被关上了"));
    });
}

/* 把 ws:// 的地址换成同一个服务的 http:// 地址。`/artifact/<id>` 和
 * `/sample` 都挂在同一个 origin 上。 */
function httpOrigin(wsUrl) {
    return wsUrl.replace(/^ws/, "http").replace(/\/ws\/?$/, "");
}

class PlayClient {
    constructor(handlers) {
        this.handlers = handlers;
        this.ws = null;
        // 连上之前是 null —— `httpOrigin(client.url)` 只在收到 hello /
        // finished 之后用,那时 connect() 已经把它填好了。
        this.url = null;
        // 还没连上时想发的消息。断线重连期间点「编译」会落到这里,
        // onopen 一冲就发出去 —— 不能丢,丢了界面会永远停在「排队中…」。
        this.pending = [];
        // LSP 消息路由回调。脚本在收到 hello 并决定启用 LSP 后才设上;
        // 在此之前 LSP 消息一律不处理 —— 服务端不应该发,但保险起见。
        this.onLsp = null;
    }

    /* 从后端池里挑一个连上,把这个 socket 接过来用。
     * 返回 true/false,不抛 —— 调用方(按钮点击)要的是「成没成」,不是异常。
     * 池子每次用之前重新打乱,顺序由 pickBackend 那边的 shuffled 给。 */
    async connect() {
        if (this.ws && this.ws.readyState <= 1) return true;
        this.handlers.onStatus("找后端…", "pending");
        let ws;
        let url;
        try {
            ws = await pickBackend(shuffled(BACKENDS));
            url = ws.url;
        } catch (e) {
            this.ws = null;
            this.handlers.onStatus(`连不上后端:${e.message}`, "err");
            return false;
        }
        // pickBackend 建 socket 时没挂 handler,这里补上。
        this.url = url;
        this.ws = ws;
        ws.onopen = () => {
            this.handlers.onStatus("已连接", "ok");
        };
        ws.onclose = () => {
            // 陈旧 socket 守卫:重连后 this.ws 已指向新连接,旧 socket 的
            // close 随后到达时不能拿它覆盖新连接刚设的状态,也不能重置按钮态。
            if (ws !== this.ws) return;
            // 断连就把待发队列清掉,**不**在重连后重放。「编译」是有副作用的
            // 动作,在用户不知情时静默占一个排队位,比丢一次没送达的点击更坏。
            this.pending = [];
            this.handlers.onStatus("连接已断开", "err");
            this.handlers.onIdle();
        };
        ws.onerror = () => {
            if (ws !== this.ws) return;
            this.handlers.onStatus("连不上后端", "err");
        };
        ws.onmessage = (ev) => {
            let m;
            try { m = JSON.parse(ev.data); } catch { return; }
            // handler 抛出来就到此为止,后面每条消息都还能收,但**这一条**
            // 的界面更新做了一半 —— 那种半吊子状态最难查(按钮藏了、状态行
            // 停在旧文案、下载链接没出来)。所以记下来摆到界面上,别只让它
            // 进 console。
            try {
                this.#dispatch(m);
            } catch (e) {
                this.handlers.onError({
                    code: "client",
                    message: `处理后端消息时出错:${e && e.message ? e.message : e}`,
                });
            }
        };
        return true;
    }

    /* 点一次「编译」= 重扫一遍池子。断线之后也走这条,所以「连接已断开」
     * 之后直接再点编译就能换个后端,不用刷新页面。
     *
     * `target` 是 `Hello.targets` 里的一项 —— 不传就让服务端走 DEFAULT_TARGET。 */
    async submit(source, target) {
        if (!(await this.connect())) return;
        // 回包之前按"排队中"显示,免得点了没反应。
        this.handlers.onStatus("排队中…", "pending");
        this.send({ type: "submit", source, target: target ?? null });
    }

    cancel() { this.send({ type: "cancel" }); }

    /* 公开的发消息接口 —— LSP / 提交 / 取消都走它。
     * readyState 不是 1 就先入队 —— 首连握手和断线重连两条路径都靠它,
     * 否则消息静默消失,界面卡在「排队中…」。 */
    send(o) {
        if (this.ws && this.ws.readyState === 1) {
            this.ws.send(JSON.stringify(o));
        } else {
            this.pending.push(o);
        }
    }

    #dispatch(m) {
        switch (m.type) {
            case "hello":
                this.handlers.onHello(m);
                break;
            case "queued":
                // 用户要的就是「队列人数/并发数」。
                this.handlers.onStatus(
                    `排队中,前面 ${m.position} 位（队列 ${m.queue}/${m.max_jobs}）`,
                    "pending");
                break;
            case "started":
                this.handlers.onStarted(m);
                break;
            case "log":
                this.handlers.onLog(m);
                break;
            case "finished":
                this.handlers.onFinished(m);
                break;
            case "cancelled":
                // 取消**也会回一条消息**。服务端那边是直接 abort 任务的,
                // 原本什么都不发 —— 于是界面永远停在「编译中…」、取消按钮
                // 一直亮着。别把这条当成可选的消息悄悄忽略掉。
                this.handlers.onCancelled(m);
                break;
            case "failed":
                this.handlers.onFailed(m);
                break;
            case "error":
                this.handlers.onError(m);
                break;
            case "lsp_ready":
            case "lsp_tokens":
            case "lsp_diagnostic":
            case "lsp_completion":
                // LSP 消息路由到客户端(若已挂上)。`onLsp` 默认 null,
                // 脚本在 hello 决定启用 LSP 之后才设上。
                if (this.onLsp) this.onLsp(m);
                break;
            default:
                // 认不出来的消息不忽略 —— 记一行。将来加 lsp 消息时,
                // 老前端就是靠这一行走过去的。
                this.handlers.onLog({ stream: "stderr", text: `未知消息 ${m.type}\n` });
        }
    }
}

/* `setEditorText` / `getEditorText` —— 把 CodeMirror 视图包成现有 UI 期望的
 * 「取文本 / 显示文本」一对方法,其它地方不用关心编辑器细节。
 * `loadSample` 和「编译」按钮都走它。 */
function makeEditorOps(view) {
    return {
        getText() { return view.state.doc.toString(); },
        setText(s) {
            view.dispatch({
                changes: { from: 0, to: view.state.doc.length, insert: s },
            });
        },
    };
}

function init() {
    const root = document.querySelector(".play");
    if (!root) return;
    // MDUI 是 web component:`.value` 之类的属性要等组件 upgrade 之后才同步,
    // 在那之前读到的都是空。整段初始化挂到 `whenDefined` 后面。
    if (customElements.get("mdui-select")) ready();
    else customElements.whenDefined("mdui-select").then(ready, (e) => {
        // mdui.js 没加载出来(unpkg 挂了/被墙)时页面就废了 —— 至少把话说明白。
        const log = root.querySelector("#play-log");
        if (log) log.textContent = `界面组件没加载出来:${e}
`;
    });
}

function ready() {
    const root = document.querySelector(".play");

    const $ = (id) => root.querySelector(`#${id}`);
    const editorHost = $("play-editor");
    /* `editable` 是 CM 的 facet,写死之后只能用 Compartment 切回。
     * 「编译中…」「还在烘」两种情况都需要把编辑器冻住 —— 写成 Compartment
     * 让 setBusy() 直接 dispatch 一行就能切换,不用重建视图。 */
    const editableCompartment = new Compartment();
    const status = $("play-status");
    const log = $("play-log");
    const runBtn = $("play-run");
    const cancelBtn = $("play-cancel");
    const dlBtn = $("play-download");
    const sampleSel = $("play-sample");
    const targetSel = $("play-target");
    const setStatus = (text, kind) => {
        status.textContent = text;
        root.dataset.status = kind || "";
    };
    const appendLog = (stream, text) => {
        log.textContent += text;
        log.scrollTop = log.scrollHeight;
        log.dataset.lastStream = stream;
    };
    const setBusy = (busy) => {
        // 两个理由都能让「编译」不可点:正在跑,或者镜像还没烘好。
        // 写成 `|| !ready` 是为了让忙碌结束之后按钮不会在 not_ready
        // 的情况下被错误地恢复成可点。
        runBtn.disabled = busy || !ready;
        cancelBtn.hidden = !busy;
        // 编辑器只读:不锁定整个组件,这样选中复制还能用。
        // `editableCompartment` 是动态开关 —— `EditorView.editable` 是 facet,
        // 在 `EditorState.create({...})` 里写死之后再 dispatch `reconfigure`
        // 才能切换。
        if (view) view.dispatch({
            effects: editableCompartment.reconfigure(
                EditorView.editable.of(!busy)
            ),
        });
    };

    /* LSP 客户端 —— 在 hello 之前不创建,初始为 null。
     * updateListener / completionProvider 都通过这个闭包变量访问,
     * null 时两个回调都安全地 no-op。 */
    let lsp = null;
    /* CodeMirror 视图 —— 总是创建,LSP 关掉时就是纯文本编辑器。 */
    let view = null;
    /* 编辑器上的文本操作句柄,外部(loadSample / runBtn click)用它。 */
    let editor = null;

    /* 补全触发时算位置:context.pos 是 CodeMirror 的绝对字符偏移;
     * 转成 LSP 的 0 基 (line, column) —— column 按 UTF-16 code unit,
     * 与 CodeMirror 默认一致,直接相减。 */
    const completionProvider = async (context) => {
        if (!lsp) return null;
        // `activateOnTyping` 是在**键入的字符之前**触发的:`context.pos` 落在
        // 刚敲下的那个字符**之前**。直接拿它当 `from` 会把已敲的字符留在
        // 替换范围之外,选中补全后就变成 `llog`(用户报的那个 bug)。
        // 自己从 pos 往回走过词字符,算出这个词的起点。
        const lineObj = context.state.doc.lineAt(context.pos);
        const before = lineObj.text.slice(0, context.pos - lineObj.from);
        const m = /[\w]*$/.exec(before);
        const typed = m ? m[0] : "";
        // 什么前缀都没有就别弹 —— RA 会把整份 crate 的符号都列出来,
        // 满屏 item 只会挡住代码(`validFor: /^[\w]*$/` 会匹配空串,
        // 所以光靠它挡不住)。
        if (typed.length === 0) return null;
        const line = lineObj.number - 1;
        const column = context.pos - lineObj.from;
        const items = await lsp.requestCompletion(line, column);
        if (!items || items.length === 0) return null;
        return {
            from: context.pos - typed.length,
            options: items.map((item) => ({
                label: item.label,
                type: kindToType(item.kind),
                detail: item.detail || undefined,
            })),
            validFor: /^[\w]+$/,
        };
    };

    /* 缩进服务。
     *
     * `@codemirror/lang-rust` **不提供** `indentation`:
     * `languageDataAt("indentation", pos)` 在装了 `rust()` 的状态里返回空数组。
     * 所以 Enter 的智能缩进没有来源 —— 光秃的编辑器里块内换行就是顶格
     * (这正是用户报的「块内换行不自动补缩进」)。
     *
     * 只做「打开就多一层、关掉就少一层」这一条,不追 Rust 的完整缩进规则:
     * 一份朴素但正确率高的缩进,比一份看起来聪明、实际在链式调用和
     * where 子上乱猜的缩进好 —— 后者会把用户的代码越排越乱。 */
    const OPENERS = "{[(";
    const CLOSERS = "}])";
    const INDENT = "    ";
    const rustIndent = {
        closedIndent: () => 0,
        indentation: (state, line) => {
            const src = state.doc.line(line - 1);
            if (!src) return 0;
            const text = src.text;
            const base = text.length - text.trimStart().length;
            const trimmed = text.trimEnd();
            const last = trimmed.slice(-1);
            if (OPENERS.includes(last)) return base + INDENT.length;
            if (CLOSERS.includes(last)) return Math.max(0, base - INDENT.length);
            return base;
        },
    };

    view = new EditorView({
        state: EditorState.create({
            doc: "",
            extensions: [
                lineNumbers(),
                highlightActiveLine(),
                closeBrackets(),
                // 初始为可编辑;setBusy() 用 editableCompartment.reconfigure 切。
                editableCompartment.of(EditorView.editable.of(true)),
                tokensField,
                diagnosticsField,
                decorationField,
                // 启用补全;override 让 completionProvider 完全控制,不被
                // 默认的 keyword/word-based 完成污染。
                autocompletion({
                    override: [completionProvider],
                    activateOnTyping: true,
                    closeOnBlur: true,
                }),
                // Tab 在 textarea 里默认是切焦点,写 Rust 的人会疯。
                // 用官方的 `indentWithTab`:它**按当前缩进**插空格(在行首
                // 补齐到下一层,中间对齐到列),而不是无脑插 4 个;并且它
                // 挂在默认优先级上,补全弹窗开着时 Tab 仍然交给弹窗选中
                // 下一项。之前用 `Prec.high` 自写 handler 抢在弹窗前面,
                // 弹窗一开 Tab 就不工作了。
                rust(),
                indentUnit.of(INDENT),
                // `indentOnInput` 让输入时保持当前缩进;`indentation` 是 Enter
                // 走 `insertNewlineAndIndent` 时查的服务。两者是同一套缩进
                // 约定的两条入口,缺一个都会让「块内换行不缩进」复现。
                indentOnInput(),
                indentation.of(rustIndent),
                // Tab 缩进。挂**默认**优先级(不是 `Prec.high`):补全弹窗
                // 开着自己的 Tab 绑定,同优先级下先注册者先跑 —— 我们写在
                // 扩展列表靠后,于是弹窗开着时 Tab 归弹窗(选中下一项),
                // 没开时才是这里。
                // 没有用官方的 `indentWithTab`:从 esm.sh 加载时它带着另一份
                // `@codemirror/view`,产出的扩展会被本页这份 `state` 判成
                // 「Unrecognized extension value」直接让整个编辑器建不起来。
                keymap.of([{
                    key: "Tab",
                    run: (v) => {
                        const { from, to } = v.state.selection.main;
                        const unit = v.state.languageDataAt("indentUnit", from) || INDENT;
                        const indent = v.state.lineAt(from).search(/\s*$/, 0, "end").from;
                        const spaces = " ".repeat(unit.length);
                        const inserts = to === from && to === v.state.doc.lineAt(from).from
                            ? spaces
                            : null;
                        v.dispatch(inserts ? {
                            changes: { from, to, insert: inserts },
                            selection: { anchor: from + inserts.length },
                        } : v.state.replaceSelection(spaces));
                        return true;
                    },
                }]),
                // 撤销/重做。显式装上 `historyKeymap`(`Mod-z` / `Mod-y` /
                // `Mod-Shift-z`),不要依赖编辑器的默认绑定 —— GitHub Pages
                // 上页面的 `Mod` 在 Windows/Linux 是 Ctrl、在 macOS 是 Cmd,
                // 少一个绑定就是「按了没反应」。
                history(),
                keymap.of(historyKeymap),
                // 文档变化 → LspClient 去抖 300 ms 后发 LspChange。
                // `lsp` 可能为 null,check 一下避免无谓调用。
                EditorView.updateListener.of((u) => {
                    if (u.docChanged && lsp) lsp.onDocChanged(u.state.doc.toString());
                }),
            ],
        }),
        parent: editorHost,
    });
    editor = makeEditorOps(view);

    // 下载地址在收到 finished 时才算得出来:后端给的是**服务端相对**
    // 路径(`/artifact/<id>`),而这一页在 GitHub Pages 上,直接用相对
    // 路径会去文档站找那个文件、必然 404。
    let downloadUrl = null;
    // 第一次连上时镜像往往还在烘。这时要每 5 秒看一眼 /status 把烘焙
    // 日志尾巴显示出来 —— 否则用户看到的就是一句"还在烘焙",不知道
    // 是卡住了还是在动。
    let ready = false;
    let bakeTimer = null;
    let tailSeen = 0;

    const stopPolling = () => {
        if (bakeTimer !== null) {
            clearInterval(bakeTimer);
            bakeTimer = null;
        }
    };
    const pollBake = () => {
        if (bakeTimer !== null || ready) return;
        bakeTimer = setInterval(async () => {
            let j;
            try {
                if (!client.url) return;
                const r = await fetch(`${httpOrigin(client.url)}/status`);
                if (!r.ok) return;
                j = await r.json();
            } catch {
                return;   // 后端还没起来,下一轮再说
            }
            const tail = (j.baking && j.baking.tail) || [];
            // 只贴新增的那几行,不然每 5 秒把整个尾巴重打一遍。
            if (tail.length > tailSeen) {
                for (const line of tail.slice(tailSeen)) appendLog("stdout", line + "\n");
                tailSeen = tail.length;
            }
            if (j.ready) {
                ready = true;
                stopPolling();
                runBtn.disabled = false;
                setStatus(`就绪 · 镜像 ${(j.serving || "").slice(0, 7)}`, "ok");
                appendLog("stdout", "── 镜像好了,可以编译了 ──\n");
            }
        }, 5000);
    };

    /* 用 `Hello.targets` 填下拉框,默认选第一项;unknown 显示名走原值。 */
    const populateTargetSelect = (targets) => {
        if (!targetSel) return;
        targetSel.textContent = "";
        for (const t of targets) {
            const opt = document.createElement("option");
            opt.value = t;
            opt.textContent = TARGET_LABELS[t] || t;
            targetSel.appendChild(opt);
        }
        // 默认第一项。下拉框列表为空时不改 value,避免 `null` 进 submit。
        if (targets.length > 0) targetSel.value = targets[0];
    };

    const client = new PlayClient({
        onStatus: setStatus,
        onHello: (m) => {
            ready = !!m.image_commit;
            // 没烘好之前按钮是灰的 —— 后端也会拒,但让用户先点再被拒
            // 是白费一次往返。
            runBtn.disabled = !ready;
            const c = ready ? m.image_commit.slice(0, 7) : "尚无";
            setStatus(`就绪 · 镜像 ${c}`, ready ? "ok" : "pending");
            if (!ready) {
                appendLog("stderr", "后端还在烘焙镜像,第一次可能要几分钟。\n");
                pollBake();
            }
            // 目标选择器由 hello 驱动 ——
            // 先把它填好,目标选择就发生在这一刻,而不是页面加载时。
            populateTargetSelect(m.targets || []);
            // 启用 LSP 客户端:hello 里有 lsp_supported 才挂上,否则就当
            // 没有 LSP 客户端,编辑框只走普通 CodeMirror(纯文本、无高亮)。
            if (m.lsp_supported) {
                // 重连会再走一次这里。上一个实例必须先销毁:它挂着的去抖
                // timer 会在新会话 lsp_open 之后才触发,拿旧内容发一条
                // lsp_change 过去。
                if (lsp) lsp.dispose();
                lsp = new LspClient({
                    view,
                    send: (msg) => client.send(msg),
                    onStatus: (text) => setStatus(text, "ok"),
                });
                client.onLsp = (msg) => lsp.onMessage(msg);
                // 发 LspOpen —— 服务端在池里建一个 RA 会话,准备好了回 LspReady。
                lsp.open();
                // 高亮要等 RA 就绪才出现,这之前给一条「分析中…」让用户
                // 知道编辑框不是坏了 —— 空黑框会让人以为是 bug。
                const base = ready ? `就绪 · 镜像 ${c}` : "就绪";
                setStatus(`${base} · 分析中…`, "pending");
            } else {
                // 服务端这次说没有 LSP(可能这次连的是没开这功能的那台)。
                // 同理要清掉上一个实例,否则它还在发消息。
                if (lsp) { lsp.dispose(); lsp = null; client.onLsp = null; }
                appendLog("stdout", "── 服务端 LSP 关闭,编辑框无高亮 ──\n");
            }
        },
        onStarted: (m) => {
            appendLog("stdout", `── 开始编译（镜像 ${m.image_commit.slice(0, 7)}）──\n`);
            setBusy(true);
            setStatus("编译中…", "pending");
        },
        onLog: (m) => appendLog(m.stream, m.text),
        onFinished: (m) => {
            setBusy(false);
            setStatus(`编译成功 · ${m.bytes} 字节`, "ok");
            // **不能**用 new URL(m.download, base):m.download 是绝对路径
            // ("/artifact/<id>"),new URL 会把它当 origin 根解析,反代前缀
            // ("/rust2genshin")会被整个丢掉。去掉开头的斜杠按字符串接上,
            // 前缀就跟着 httpOrigin 走。
            downloadUrl = httpOrigin(client.url) + "/" + m.download.replace(/^\/+/, "");
            dlBtn.hidden = false;
        },
        onCancelled: () => {
            // 和 finished/failed 一样收摊。状态不标红:取消是用户自己按的,
            // 不是错误。
            setBusy(false);
            setStatus("已取消", "");
            appendLog("stdout", "── 已取消 ──\n");
        },
        onFailed: (m) => {
            setBusy(false);
            setStatus(m.message || "编译失败", "err");
            appendLog("stderr", `\n${m.message}\n`);
        },
        onError: (m) => {
            setBusy(false);
            // ip_busy 那条后端给的就是中文原文,直接显示。
            setStatus(m.message, m.code === "ip_busy" ? "warn" : "err");
        },
        onIdle: () => {
            // 连接断了就别再轮询 /status —— 那是在冲一个没人应答的
            // 后端每 5 秒发一次请求。
            stopPolling();
            setBusy(false);
            // 断线时**必须**改状态行。`setBusy(false)` 只藏按钮,不动文字 ——
            // 于是「编译中…」会一直挂着,而 `finished` 已经跟着断掉的连接
            // 一起没了。用户看到的就是永远编译中、没有下载按钮,完全猜不出
            // 是断了。`ws.onclose` 里那句「连接已断开」靠不住:重连之后旧
            // socket 的 close 才到,会被那边的陈旧 socket 守卫吞掉。
            // 这条是最后一道,不管前面有没有已经报过更好的原因都照实说。
            if (root.dataset.status === "pending") {
                setStatus("连接已断开,这次的结果没收到 —— 点「编译」重试", "err");
            }
        },
    });

    runBtn.addEventListener("click", () => {
        log.textContent = "";
        dlBtn.hidden = true;
        downloadUrl = "";
        const target = targetSel ? targetSel.value : null;
        client.submit(editor.getText(), target);
    });
    cancelBtn.addEventListener("click", () => client.cancel());
    dlBtn.addEventListener("click", () => {
        if (!downloadUrl) return;
        // 只校验 scheme,不比较 origin。downloadUrl 是在 finished 时按
        // 字符串拼出来的(见上面那处),后端可以完全控制 m.download ——
        // 传 "javascript:alert(1)" 拼出来就是 javascript: 开头的地址。
        // 这里用 new URL 反解一遍,protocol 不是 http(s) 就拒。
        // 不按 origin 比:页面 origin 与后端 origin 本来就不同(本地
        // 3000 vs 后端 8787,线上 github.io vs mzverse.org),比了就把
        // 正常下载全挡死。
        const u = new URL(downloadUrl);
        if (u.protocol !== "http:" && u.protocol !== "https:") {
            appendLog("stderr", "拒绝下载:链接不是 http(s)\n");
            return;
        }
        window.location.href = downloadUrl;
    });

    // ---- 选项卡:日志 / 预览 ----
    // 两个面板都留在 DOM 里,切换只动 `hidden` —— 这样切走再切回来,日志
    // 里已经滚出来的内容还在,不用重跑就能对着报错看代码。
    const tabs = root.querySelectorAll(".play-tab");
    const panels = {
        log: $("play-panel-log"),
        preview: $("play-panel-preview"),
    };
    tabs.forEach((tab) => {
        tab.addEventListener("click", () => {
            const want = tab.dataset.tab;
            tabs.forEach((t) => t.toggleAttribute("data-active", t === tab));
            for (const [name, panel] of Object.entries(panels)) {
                panel.hidden = name !== want;
            }
        });
    });

    // ---- 拖动中间那条改左右宽度 ----
    const layout = $("play-layout");
    const splitter = $("play-splitter");
    // 两边都不许窄过这个值:拖到底时要是能压成 0 宽,那一栏就再也抓不回来
    // 了(拖动条还贴在最边上,但滑块本身也只剩 8px)。
    const MIN_PANE = 220;
    const SPLITTER_W = 8;
    let dragging = false;

    const clampLeft = (x) => {
        const total = layout.getBoundingClientRect().width;
        return Math.max(MIN_PANE, Math.min(x, total - MIN_PANE - SPLITTER_W));
    };
    splitter.addEventListener("pointerdown", (e) => {
        dragging = true;
        splitter.setAttribute("data-dragging", "");
        // 指针捕获之后,即使拖出拖动条本身,pointermove 也还发到这上面 ——
        // 否则手一快滑出去就断了。
        splitter.setPointerCapture(e.pointerId);
        // 不然整页文字会被框选,光标也变成插入符。
        document.body.style.userSelect = "none";
        e.preventDefault();
    });
    splitter.addEventListener("pointermove", (e) => {
        if (!dragging) return;
        const left = e.clientX - layout.getBoundingClientRect().left;
        layout.style.setProperty("--play-left", `${Math.round(clampLeft(left))}px`);
    });
    const endDrag = (e) => {
        if (!dragging) return;
        dragging = false;
        splitter.removeAttribute("data-dragging");
        document.body.style.userSelect = "";
        try {
            splitter.releasePointerCapture(e.pointerId);
        } catch {
            /* 指针已经释放过了,抛了也无所谓 */
        }
    };
    splitter.addEventListener("pointerup", endDrag);
    splitter.addEventListener("pointercancel", endDrag);
    // 窗口缩小之后那个固定像素宽会把右边挤没,重新夹一次。没拖过时
    // `--play-left` 还是默认的 `1fr`,不该被改写成 px。
    window.addEventListener("resize", () => {
        const cur = layout.style.getPropertyValue("--play-left");
        if (!cur.endsWith("px")) return;
        layout.style.setProperty(
            "--play-left",
            `${Math.round(clampLeft(parseFloat(cur)))}px`
        );
    });

    // 样例是**静态资产**(play/demo/*.rs),不从后端取。datalist 是随键入
    // 过滤的,change 就去 fetch 会在打字的每个字符上发一次请求,所以点
    // 「载入」才取。
    const loadSample = async () => {
        const path = sampleSel.value;
        // 没选(停在「选一个…」那条)就别动编辑器 —— 拿空路径去 fetch 会清空
        // 人正在写的代码。
        if (!path) return;
        try {
            const r = await fetch(path);
            if (!r.ok) throw new Error(`HTTP ${r.status}`);
            editor.setText(await r.text());
        } catch (err) {
            appendLog("stderr", `取不到样例 ${path}:${err.message}\n`);
        }
    };
    $("play-sample-load").addEventListener("click", loadSample);
    // 进来就填上默认选中的那个(下拉里第一项)。空着一个编辑器让人对着
    // 空白犯愣,而选都选好了。
    loadSample();

    client.connect();
}

// ESM 模块加载时 init() 由模板里的 `<script type="module">` 触发 ——
// 模块本身就是 defer,DOM 就绪在执行前已经完成。直接跑,不再等 DOMContentLoaded。
init();