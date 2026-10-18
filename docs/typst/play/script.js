/* 摆弄页。
 *
 * 编辑器是 CodeMirror 6;语义高亮 / 诊断 /
 * 补全的候选都由 RA 经后端 WebSocket 给,本地不做语义分析 —— 不存在「高亮
 * 和分析结论不一致」(Review Focus 第 5 条)。补全项拿到手后只把 RA 给的
 * 签名字符串拆成参数/返回类型用于展示与光标落点(见 `parseSignature`)。
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
import { defaultKeymap, history, historyKeymap } from "https://esm.sh/@codemirror/commands@6";
import { autocompletion, closeBrackets } from "https://esm.sh/@codemirror/autocomplete@6";
import { indentService, indentUnit } from "https://esm.sh/@codemirror/language@6";
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

/* 打字触发的后台重连失败后的冷却。显式点「编译」不受这个限制。 */
const RECONNECT_BACKOFF_MS = 5000;

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
        // 正在进行的重连。打字会连续触发 `ensureConnected`,不合并的话
        // 一次断线会同时开出一堆 socket,谁最后 resolve 谁赢 —— 其余的全
        // 泄漏成幽灵连接。
        this._connecting = null;
        // 上次连失败的时间,用来做退避。后端全挂时键盘每次停顿都重连
        // 一遍,只会把 4 s 超时叠成一串。
        this._lastFailAt = 0;
    }

    /* 从后端池里挑一个连上,把这个 socket 接过来用。
     * 返回 true/false,不抛 —— 调用方(按钮点击)要的是「成没成」,不是异常。
     * 池子每次用之前重新打乱,顺序由 pickBackend 那边的 shuffled 给。
     *
     * `force` 是给「编译」按钮用的:用户明确点在重连,退避不该拦它。
     * 打字触发的 `ensureConnected` 走非 force,失败后安静等一会儿。 */
    async connect(force = false) {
        if (this.ws && this.ws.readyState === 1) return true;
        // 已经有重连在跑:复用同一个 Promise,别再开第二条连接。
        if (this._connecting) return this._connecting;
        if (!force && Date.now() - this._lastFailAt < RECONNECT_BACKOFF_MS) return false;
        this._connecting = this._connect();
        try {
            return await this._connecting;
        } finally {
            this._connecting = null;
        }
    }

    /* 后台重连。打字时调用 —— 用户只是在写代码,不能让它等一个 Promise,
     * 成败都由 `connect` 里的状态更新告诉用户。 */
    ensureConnected() {
        // 失败已经由 `connect` 写到状态行了,这里吞掉 rejection 就够了。
        this.connect().catch(() => {});
    }

    async _connect() {
        this.handlers.onStatus("找后端…", "pending");
        let ws;
        let url;
        try {
            ws = await pickBackend(shuffled(BACKENDS));
            url = ws.url;
        } catch (e) {
            this._lastFailAt = Date.now();
            this.ws = null;
            this.handlers.onStatus(`连不上后端:${e.message}`, "err");
            return false;
        }
        // pickBackend 建 socket 时没挂 handler,这里补上。
        this.url = url;
        this.ws = ws;
        // 注意:`pickBackend` 只在 open 之后才 resolve,所以这个 socket
        // 已经是 OPEN 的,`ws.onopen` 不会再来 —— 状态和待发队列在这里
        // 直接处理。
        ws.onclose = () => {
            // 陈旧 socket 守卫:重连后 this.ws 已指向新连接,旧 socket 的
            // close 随后到达时不能拿它覆盖新连接刚设的状态,也不能重置按钮态。
            if (ws !== this.ws) return;
            this.ws = null;
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
        this._lastFailAt = 0;
        this.handlers.onStatus("已连接", "ok");
        // socket 已经开着,攒下的非 LSP 消息(取消等)现在可以发了。
        this.#flushPending();
        return true;
    }

    /* 把断线期间攒下的消息补发出去。只在 socket 确认 OPEN 时调用;
     * 走裸 `ws.send` 而不是 `send`,免得又绕回队列。 */
    #flushPending() {
        if (!this.ws || this.ws.readyState !== 1) return;
        const queued = this.pending;
        this.pending = [];
        for (const o of queued) {
            try { this.ws.send(JSON.stringify(o)); } catch { /* 又断了:丢掉 */ }
        }
    }

    /* 点一次「编译」= 重扫一遍池子。断线之后也走这条,所以「连接已断开」
     * 之后直接再点编译就能换个后端,不用刷新页面。
     *
     * `target` 是 `Hello.targets` 里的一项 —— 不传就让服务端走 DEFAULT_TARGET。 */
    async submit(source, target) {
        // 点了「编译」就是明确要连,不吃打字重连的退避。
        if (!(await this.connect(true))) return;
        // 回包之前按"排队中"显示,免得点了没反应。
        this.handlers.onStatus("排队中…", "pending");
        this.send({ type: "submit", source, target: target ?? null });
    }

    cancel() { this.send({ type: "cancel" }); }

    /* 公开的发消息接口 —— LSP / 提交 / 取消都走它。
     * readyState 不是 1 就顺手拉一次后台重连:空闲一段时间后后端会关掉
     * 连接,用户下次打字(tokens / 补全 / lsp_change)就是这条路径 ——
     * 以前这里只入队不重连,于是必须点一次「编译」才活过来。
     *
     * LSP 消息**不入队**:重连后服务端会重新 hello,`onHello` 会新建
     * `LspClient`、重发 `lsp_open` 并整份同步文档。旧会话的 `lsp_change` /
     * `lsp_completion` 到新会话里要么撞 BadRequest,要么 request_id 对不上。
     * 其余消息(取消、提交)保留入队语义。 */
    send(o) {
        if (this.ws && this.ws.readyState === 1) {
            this.ws.send(JSON.stringify(o));
            return;
        }
        if (typeof o.type === "string" && !o.type.startsWith("lsp_")) {
            this.pending.push(o);
        }
        this.ensureConnected();
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
            case "lsp_diagnostics":
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
    // 不再等组件 upgrade。两个下拉都是原生 `<select>`,`.value` 一开始就是
    // 准的;按钮这边脚本只碰 `disabled` / `hidden` —— Lit 会把 upgrade 之前
    // 设在实例上的属性接过去,而 Shoelace 的模块万一被墙,`<sl-button>` 退回
    // 成一个普通的内联元素,点击照样能用,比整页卡在等组件上强。
    ready();
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
    /* 断线重连的触发点。编辑器在 `client` 建好之前就存在,所以先占位,
     * 等 PlayClient 构造完再接上 —— 直接在 updateListener 里引用 `const
     * client` 要赌「初始化后才会触发更新」,没必要。 */
    let hintReconnect = null;
    /* CodeMirror 视图 —— 总是创建,LSP 关掉时就是纯文本编辑器。 */
    let view = null;
    /* 编辑器上的文本操作句柄,外部(loadSample / runBtn click)用它。 */
    let editor = null;

    /* ---------- 补全项的签名解析 ---------- */

    /* RA 的签名是给人看的字符串(实测样例):
     *     `const fn(self) -> u32`
     *     `fn(&self, i32) -> i32`
     *     `fn(impl ToString)`
     * 注意它**只给类型、不给参数名**,所以 `ilog` 显示成 `ilog(self, i32)`,
     * 不是 `ilog(self, rhs: i32)`。要参数名得再发 signatureHelp / hover,
     * 每个候选一次往返,不划算。
     *
     * 返回 `null` 表示「不是可解析的函数签名」—— 调用方退回只插名字,
     * 不补括号,不会把代码弄坏。 */
    /* RA 会在签名前面挂一堆修饰:`pub` / `pub(crate)` / `const` /
     * `unsafe` / `async` / `extern "C"` / `#[attr]`。
     *
     * 属性那一项尤其碍眼:RA 嫌长会截断成 `#[unsafe(…)]` —— 省略号是
     * **RA 自己**的截断标记,不是用户代码里的东西,插进补全列表只会让人
     * 以为有个叫 `…` 的写法。而且 `parseSignature` 本来就只认 `fn`,
     * 这些前缀一个都用不上,直接剥干净。
     *
     * 只剥前缀,不动参数里的属性(惯例上不会出现在 `fn(...)` 内部,真出现
     * 了也是签名的一部分)。`#[...]` 按方括号配对删,免得 `#[a(b[c])]`
     * 提前收尾。 */
    const stripPrefixNoise = (detail) => {
        let s = detail.trim();
        for (;;) {
            const before = s;
            if (s.startsWith("#[")) {
                let depth = 0;
                let end = -1;
                for (let i = 1; i < s.length; i++) {
                    if (s[i] === "[") depth++;
                    else if (s[i] === "]") {
                        depth--;
                        if (depth === 0) { end = i + 1; break; }
                    }
                }
                // 没闭合(RA 截断到了属性中间):后面全是噪音,整条丢。
                if (end < 0) return "";
                s = s.slice(end).trim();
            } else {
                // 修饰符可能连着来(`pub unsafe extern "C" fn`),一个个摘。
                const m = /^(?:pub(?:\s*\([^)]*\))?|const|async|unsafe|extern(?:\s+"[^"]*")?|default)\s+/.exec(s);
                if (m) s = s.slice(m[0].length).trim();
            }
            if (s === before) break;
        }
        return s;
    };

    const parseSignature = (raw) => {
        if (!raw) return null;
        const detail = stripPrefixNoise(raw);
        // 剥完可能什么都不剩(整条 detail 都是属性)。
        if (!detail) return null;
        // 定位声明里的 `fn`:detail 一定是签名,第一个 `fn` 就是它。
        const fnAt = detail.search(/\bfn\b/);
        if (fnAt < 0) return null;
        // 从 `fn` 往后找**顶层**的左括号。泛型参数里的逗号/括号
        // (`fn<T: Into<U>>(x: T)`)不能算进参数列表。
        let angle = 0;
        let open = -1;
        for (let i = fnAt + 2; i < detail.length; i++) {
            const c = detail[i];
            if (c === "<") angle++;
            else if (c === ">" && detail[i - 1] !== "-") angle--;
            else if (c === "(" && angle <= 0) { open = i; break; }
        }
        if (open < 0) return null;
        // 只数圆括号:type 里的 `Fn(A) -> B`、元组都自平衡。
        let depth = 0;
        let close = -1;
        for (let i = open; i < detail.length; i++) {
            if (detail[i] === "(") depth++;
            else if (detail[i] === ")") {
                depth--;
                if (depth === 0) { close = i; break; }
            }
        }
        if (close < 0) return null;
        const params = splitTopLevel(detail.slice(open + 1, close));
        // RA 嫌长时会截断,统一留个 `…`(`#[unsafe(…)]`、`fn(a, …)`、
        // `-> (A, B, …)`)。参数位置出现这个就意味着**实参个数不可知** ——
        // 光标落点取决于有没有实参,猜不得,整条退回不可解析,让调用方
        // 只插名字不补括号(见 `applyCallable`)。
        if (params.some(isEllipsis)) return null;
        const rest = detail.slice(close + 1).trim();
        const ret = /^->\s*(.+)$/.exec(rest);
        const retText = ret ? ret[1].trim() : null;
        return {
            params,
            ret: retText,
            // 返回类型被 RA 截断(`-> (A, B, …)`)。削掉省略号会留下
            // `(A, B,` 这种半截类型,比不显示更糟;显示成 `()` 又会被读
            // 成「没有返回值」。老老实实什么都不显示 —— 右边空着总比
            // 显示错的东西好。参数那侧同样处理不了(实参个数不可知,
            // 光标落点取决于它),所以整条退回不可解析。
            retTruncated: !!retText && retText.includes("…"),
        };
    };

    /* RA 的省略号标记。两种完全不同的用法,别混:
     *   - `fn(a, …)` / `-> (A, B, …)`:`detail` 里,**RA 截断了输出**。
     *     省略号是「后面还有,被砍掉了」,不是能写进代码的东西。
     *   - `unsafe(…)` / `doc = "…"`:`label` 里,**RA 的占位提示**,
     *     意思是「这里要你自己填」。`#[un` 补全出 `unsafe(…)` 就是这种。
     *
     * 后者出现在 label 而非 detail,是 `label.includes("…")` 能一眼分开
     * 的判据 —— 见 `completionProvider` 里的占位分支。 */
    const isEllipsis = (s) => /^\s*…\s*$/.test(s);

    /* 按**顶层**逗号切参数。`Option<(A, B)>`、`impl Fn(i32, i32)`、
     * `fn(i32) -> i32` 里的逗号都在更深一层,不能切。 */
    const splitTopLevel = (s) => {
        const out = [];
        let depth = 0;
        let start = 0;
        for (let i = 0; i < s.length; i++) {
            const c = s[i];
            if (c === "(" || c === "[" || c === "{" || c === "<") depth++;
            else if (c === ")" || c === "]" || c === "}") depth--;
            else if (c === ">" && s[i - 1] !== "-") depth--;
            else if (c === "," && depth === 0) {
                out.push(s.slice(start, i).trim());
                start = i + 1;
            }
        }
        const last = s.slice(start).trim();
        if (last) out.push(last);
        return out.filter(Boolean);
    };

    /* 第一个参数是不是接收者。RA 在 `.` 和 `::` 两种上下文里都写成
     * `fn(self)` —— 所以「self 算不算实参」只能由调用语法决定:
     * `.` 调用时它由左侧提供,`i32::abs` 时它要自己传。 */
    const isSelfParam = (p) =>
        /^(?:&\s*(?:'[^']+\s*)?(?:mut\s+)?)?self(?:\s*:.*)?$/.test(p);

    /* `labelDetails.detail` 里的展示注释 → 右侧的附加信息。
     * RA 在声明 `labelDetailsSupport` 后把 `(as Trait)` 放这里;
     * 别名是 `(alias popcount, popcnt)`。 */
    const completionNote = (item) => {
        const raw = item.label_details && item.label_details.detail;
        if (raw) {
            // 别名列表也可能被 RA 截断成 `(alias a, …)`。省略号在这里同样
            // 纯属噪音,削掉。
            const text = raw.trim().replace(/…\s*$/, "").trim();
            const asTrait = /^\(as\s+(.+)\)$/.exec(text);
            if (asTrait) return "of " + asTrait[1];
            const alias = /^\(alias\s+(.+)\)$/.exec(text);
            if (alias) return "alias " + alias[1];
            return text;
        }
        // 旧服务端没声明 `labelDetailsSupport`:RA 把同样的注解拼在
        // `label` 末尾,例如 `tfoo(as Tr)`、`count_ones(alias popcount,
        // popcnt)`。这里解出来,免得非要先重编服务端才能看到 `of Trait`。
        const packed = /\((as|alias)\s+([^)]*)\)$/.exec(item.label);
        if (!packed) return null;
        return packed[1] === "as" ? "of " + packed[2] : "alias " + packed[2];
    };

    /* 属性占位项的插入:省略号删掉,光标落到它原来的偏移。
     *
     * `#[un` → `#[unsafe()`,光标在括号里;`doc = "…"` → `doc = ""`,
     * 光标在引号里。嵌套同样成立 —— `doc(alias = "…")` 的省略号在内层
     * 引号中间,光标就落那儿。 */
    const applyAt = (view, completion, from, to) => {
        const text = completion.insertText || completion.label;
        view.dispatch({
            changes: { from, to, insert: text },
            selection: { anchor: from + completion.ellipsisAt },
            userEvent: "input.complete",
        });
    };

    /* 函数/方法的自定义插入:补 `()` 并把光标放进去;无实参时放到
     * 括号后。`from`/`to` 是 CodeMirror 算好的替换范围(已含用户
     * 敲下的前缀),所以直接整段替换成名字 + 括号。 */
    const applyCallable = (view, completion, from, to) => {
        // `insertText` 是剥掉展示注解后的名字;旧服务端会把 `(as Trait)`
        // 拼在 label 里,绝不能让它进代码。
        const name = completion.insertText || completion.label;
        const text = name + (completion.insertParens ? "()" : "");
        const anchor = completion.cursorInside
            ? from + name.length + 1
            : from + text.length;
        view.dispatch({
            changes: { from, to, insert: text },
            selection: { anchor },
            userEvent: "input.complete",
        });
    };

    /* 补全行拆成四段,各由 `addToOptions` 按 position 排:
     *   - label(position 50):名字,黑色 —— 这是真正会被插入的文本;
     *   - 参数(position 55):`(self, i32)`,淡色,紧贴名字;
     *   - note(position 60):`of Trait` / `alias xxx`,淡色,紧跟参数;
     *   - 返回类型(position 90):淡色,`margin-left:auto` 推到最右。
     * 参数不放进 label 是因为 CodeMirror 的 `displayLabel` 只能整段一个
     * 文本节点,没法只把括号部分染灰。 */
    const renderCompletionParams = (completion) => {
        if (!completion.params) return null;
        const el = document.createElement("span");
        el.className = "cm-completionParams";
        el.textContent = "(" + completion.params.join(", ") + ")";
        return el;
    };
    const renderCompletionNote = (completion) => {
        if (!completion.note) return null;
        const el = document.createElement("span");
        el.className = "cm-completionNote";
        el.textContent = completion.note;
        return el;
    };
    const renderCompletionRet = (completion) => {
        if (!completion.ret) return null;
        const el = document.createElement("span");
        el.className = "cm-completionRet";
        el.textContent = completion.ret;
        return el;
    };

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
        // 三条触发路径:
        //   - 有单词前缀(正常打字)
        //   - 光标紧跟 `.` 或 `::`(成员/路径补全,此时前缀为空)
        //   - 用户显式按键(Ctrl+Space)—— 这时空前缀也是在要列表
        // 其余情况什么前缀都没有就别弹:RA 会把整份 crate 的符号都列出来,
        // 满屏 item 只会挡住代码(`validFor` 匹配空串,光靠它挡不住)。
        const afterTrigger = /(?:\.|::)$/.test(before);
        if (typed.length === 0 && !afterTrigger && !context.explicit) return null;
        const line = lineObj.number - 1;
        const column = context.pos - lineObj.from;
        const items = await lsp.requestCompletion(line, column);
        if (!items || items.length === 0) return null;
        // 前缀之前那个字符决定调用语法:`.foo` 的 self 由接收者提供,
        // `Type::foo` 的 self 要作为第一个实参传。
        const beforePrefix = before.slice(0, before.length - typed.length);
        const isDotCall = /\.$/.test(beforePrefix);
        return {
            from: context.pos - typed.length,
            options: items.map((item) => {
                // 属性占位项:`unsafe(…)` / `cfg(…)` / `doc = "…"`。
                // RA 把省略号直接写在 **label** 里当「这里你自己填」的提示,
                // kind 也是 3(Function) —— 不截住它就会把 `…` 原样插进
                // 代码。规则很简单:省略号落在哪,插完光标就去哪,`…` 本身
                // 删掉。`doc(alias = "…")` 这种嵌套的也对:光标落到引号
                // 中间。
                const ellAt = item.label.indexOf("…");
                if (ellAt >= 0) {
                    const text = item.label.slice(0, ellAt) +
                        item.label.slice(ellAt + 1);
                    return {
                        label: item.label,
                        displayLabel: text,
                        type: kindToType(item.kind),
                        insertText: text,
                        params: null,
                        note: null,
                        ret: null,
                        // 走自定义插入:CM 默认会拿 `label` 或
                        // `insertText` 填,但不会把光标落到省略号原来的
                        // 位置。
                        apply: applyAt,
                        ellipsisAt: ellAt,
                    };
                }
                // 只有函数/方法(2=Method,3=Function)补括号;模块、
                // 常量、类型保持原来的纯名字插入。
                const callable = item.kind === 2 || item.kind === 3;
                // 服务端没声明 labelDetailsSupport 时 RA 把 `(as Trait)` /
                // 别名拼在 label 末尾;插入前剥掉。新服务端下这是 no-op。
                const name = callable
                    ? item.label.replace(/\s*\((?:as|alias)\s+[^)]*\)$/, "")
                    : item.label;
                const sig = callable
                    ? parseSignature(item.detail ||
                        (item.label_details && item.label_details.description))
                    : null;
                // 解析不出签名时退回普通项:不补括号,detail 原样给 CM。
                const args = sig
                    ? (isDotCall && sig.params.length > 0 && isSelfParam(sig.params[0])
                        ? sig.params.length - 1
                        : sig.params.length)
                    : 0;
                // 剥完前缀可能只剩空串(`detail` 整条都是属性),别给 CM
                // 传空 detail —— 它会画出一个空的 `.cm-completionDetail`。
                const rawDetail = item.detail ||
                    (item.label_details && item.label_details.description);
                const plainDetail = rawDetail
                    ? stripPrefixNoise(rawDetail).replace(/…\s*$/, "").trim()
                    : "";
                return {
                    // `label` 仍用于匹配(CM 拿它跟已输入前缀比对),
                    // `displayLabel` 是真正画出来的名字 —— 剥掉旧服务端
                    // 拼进来的 `(as Trait)` / `(alias ...)`。
                    label: item.label,
                    displayLabel: name,
                    type: kindToType(item.kind),
                    detail: sig ? undefined : (plainDetail || undefined),
                    // 非可调用项通常就是 label;只有剥过注解时才需要显式
                    // 覆盖,否则 CM 的默认插入会带上 `(as Trait)`。
                    insertText: callable || name !== item.label ? name : undefined,
                    // addToOptions 的三段 render 读这些自定义字段。
                    params: sig ? sig.params : null,
                    note: completionNote(item),
                    // 返回类型被 RA 截断时什么都不显示,也不能退回 `()` ——
                    // 那会被读成「无返回值」。
                    ret: sig
                        ? (sig.retTruncated ? null : (sig.ret || "()"))
                        : null,
                    cursorInside: args > 0,
                    insertParens: !!sig,
                    // 可调用项一律走自定义插入:即使签名解析失败,也要把
                    // 旧服务端拼在 label 里的注解挡在代码之外。
                    apply: callable ? applyCallable : undefined,
                };
            }),
            // 允许空串:`.` / `::` 触发时前缀本来就是空的。出现 `:` 等
            // 非单词字符时 validFor 失配,CM 会重新问一次服务端。
            validFor: /^[\w]*$/,
        };
    };

    /* 缩进服务。
     *
     * CodeMirror 只提供**机制**(`indentService` facet + `defaultKeymap` 里的
     * Enter 命令 + 语法树兜底),每种语言的缩进规则由各自的 language 包给。
     * `@codemirror/lang-rust` 一条都没给:`languageDataAt("indentService")`
     * 在装了 `rust()` 的状态里是空数组,lezer-rust 也没定义 `indentNodeProp`,
     * 于是语法树兜底算出来是 0 —— 光秃秃的编辑器里块内换行就是顶格。
     * 所以这十条规则得自己写。
     *
     * 只做「开括号多一层、其余沿用本行」,不追 Rust 的完整缩进规则:
     * 一份朴素但正确率高的缩进,比一份看起来聪明、实际在链式调用和
     * where 子上乱猜的缩进好 —— 后者会把用户的代码越排越乱。 */
    const OPENERS = "{[(";
    const INDENT = "    ";
    /* 缩进服务。签名是 `(context, pos)`,契约来自 `@codemirror/commands`
     * 的 `newlineAndIndent`:Enter 时它拿**光标位置**调这里问新行的缩进;
     * 被顶下去的 `}` 不经过这里,它用 `cx.lineIndent(line.from, -1)` 取
     * `{` 行自己的前导空白。`context.unit` 是**数字**(4),返回值是**列数**。
     *
     * 规则只有两条,关键是第一条:
     *   - 光标前**还没有非空白内容**(空行 / 新建行) → 沿用本行已有的缩进。
     *     少了这条会出事:反复回车时新行是 `"    "`,`trimEnd()` 是空串,
     *     若拿 `includes("")` 判开括号,JS 里 `"abc".includes("")` 恒为
     *     true,于是每按一次回车就多一层(4 → 8 → 12 …)。
     *   - 否则看前一个非空白字符:开括号多一层,其余沿用本行缩进(行尾的
     *     `}` 也是沿用 —— 那一行本来就在正确的层级上)。
     *
     * 只看**光标之前**那段,不看整行:光标常停在 `}` 之前,看整行会先
     * 撞上那个 `}`。 */
    const rustIndent = (context, pos) => {
        const state = context.state;
        // 越界的位置不该让 Enter 抛 —— 缩进退化成 0。
        if (!(pos >= 0 && pos <= state.doc.length)) return 0;
        const line = state.doc.lineAt(pos);
        const before = line.text.slice(0, pos - line.from);
        const base = context.countColumn(/^[ \t]*/.exec(before)[0]);
        const trimmed = before.trimEnd();
        if (trimmed === "") return base;
        return OPENERS.includes(trimmed.slice(-1)) ? base + context.unit : base;
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
                    // CM 内置的 label / detail 位置是 50 / 80。参数与
                    // trait 注释塞在 55 / 60(紧贴名字),返回类型放 90
                    // (由 CSS 推到行尾)。
                    addToOptions: [
                        { position: 55, render: renderCompletionParams },
                        { position: 60, render: renderCompletionNote },
                        { position: 90, render: renderCompletionRet },
                    ],
                }),
                // `defaultKeymap` 必须在:Enter 走的是里面的
                // `insertNewlineAndIndent`,而**只有那条命令会去问缩进服务**
                // —— 不绑它,Enter 就是浏览器原生的插一个 `\n`,缩进服务
                // 一次都不会被问。附带好处是补齐了 Backspace 合并行、
                // 光标处引号成对删除等一整批默认行为。
                // 注意它里面不含 Tab 绑定(Tab 在 `indentWithTab` 里,而那个
                // 不能用,见下),所以和我们自己的 Tab handler 不打架。
                keymap.of(defaultKeymap),
                rust(),
                indentUnit.of(INDENT),
                // 缩进只有 `indentService` 这一条真入口。`indentOnInput` 在这儿
                // 是**空转**:实测它的实现是 transactionFilter,只在
                // `tr.isUserEvent("input.type")` 时跑,还要
                // `languageDataAt("indentOnInput")` 提供一个函数 ——
                // `@codemirror/lang-rust` 不提供,配了也不触发。
                indentService.of(rustIndent),
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
                        // `languageDataAt` 返回的是**数组**(可能多层配置),
                        // 直接拿它当字符串用会得到 `"    "` 的长度 5 之类。
                        const unit = v.state.languageDataAt("indentUnit", from)[0] || INDENT;
                        const line = v.state.doc.lineAt(from);
                        const base = line.text.match(/^\s*/)[0];
                        const col = from - line.from;
                        let insert;
                        if (to === from && col <= base.length) {
                            // 还在行首的空白里:补到**下一层**,而不是加一格。
                            insert = base + unit;
                        } else {
                            // 行中间:对齐到下一个制表位。
                            const rest = col % unit.length;
                            insert = " ".repeat(rest === 0 ? unit.length : unit.length - rest);
                        }
                        v.dispatch({
                            changes: { from, to, insert },
                            selection: { anchor: from + insert.length },
                        });
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
                    if (!u.docChanged) return;
                    // 空闲后服务端会关掉连接。用户下一次打字就是「再次操作」,
                    // 在这里把连接拉回来 —— 不能等 LSP 的 _ready 分支,更不能
                    // 让用户以为必须点「编译」才能恢复。
                    if (hintReconnect) hintReconnect();
                    if (lsp) lsp.onDocChanged(u.state.doc.toString());
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

    /* 目标选择记在 localStorage 里,刷新后回到上次选的那个。
     *
     * 存的值**不写死信任**:每次 `hello` 都用当次 `Hello.targets` 校验 ——
     * 后端换了、某个目标下线了,存下来的那个可能根本不在列表里,直接套用
     * 会让 `submit` 带一个服务端不认的 target(轻则 BadRequest,重则静默
     * 编译错目标)。不在列表里就退回第一项。 */
    const TARGET_STORE_KEY = "r2g.play.target";

    /* 用 `Hello.targets` 填下拉框,优先回到上次选的;unknown 显示名走原值。 */
    const populateTargetSelect = (targets) => {
        if (!targetSel) return;
        targetSel.textContent = "";
        for (const t of targets) {
            const opt = document.createElement("option");
            opt.value = t;
            opt.textContent = TARGET_LABELS[t] || t;
            targetSel.appendChild(opt);
        }
        if (targets.length === 0) return;
        let want = null;
        try { want = localStorage.getItem(TARGET_STORE_KEY); } catch { /* 无痕模式等 */ }
        targetSel.value = want && targets.includes(want) ? want : targets[0];
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
    // 编辑器打字 → 后台重连(见 updateListener 里的说明)。
    hintReconnect = () => client.ensureConnected();

    // 目标选择写进 localStorage(见 populateTargetSelect 的注释)。
    if (targetSel) {
        targetSel.addEventListener("change", () => {
            try {
                localStorage.setItem(TARGET_STORE_KEY, targetSel.value);
            } catch {
                /* 存不了就当没这回事:下拉框本身照常工作 */
            }
        });
    }

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