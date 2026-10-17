/* 摆弄页。
 *
 * 拆掉自定义组件之前是 <playground-app> + shadow root,现在就是普通的
 * DOM:`.play` 容器 + 一堆 id。分成两个对象还是刻意的:
 *   - PlayEditor  只认「取文本 / 显示文本」几个方法,将来换 CodeMirror
 *                 或加 rust-analyzer 的补全,只动它。
 *   - PlayClient  只认「连上 / 发 submit / 收消息」,不知道编辑器存在。
 * 两者在 `init()` 里通过那几个回调接起来。
 *
 * 后端地址在 localStorage 里;没有就用 `.play` 上的 `data-backend`
 * (站点在 index.typ 里给),再没有才退回本机的 8787。
 */

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

class PlayEditor {
    constructor(textarea) {
        this.textarea = textarea;
        this.textarea.addEventListener("keydown", (e) => {
            // Tab 在 textarea 里默认是切焦点,写 Rust 的人会疯。
            if (e.key === "Tab") {
                e.preventDefault();
                const s = this.textarea.selectionStart;
                const e2 = this.textarea.selectionEnd;
                const v = this.textarea.value;
                this.textarea.value = v.slice(0, s) + "    " + v.slice(e2);
                this.textarea.selectionStart = this.textarea.selectionEnd = s + 4;
            }
        });
    }
    getText() { return this.textarea.value; }
    setText(t) { this.textarea.value = t; }
    setEnabled(on) { this.textarea.readOnly = !on; }
}

class PlayClient {
    constructor(url, handlers) {
        this.url = url;
        this.handlers = handlers;
        this.ws = null;
        this.url = null;
        // 还没连上时想发的消息。断线重连期间点「编译」会落到这里,
        // onopen 一冲就发出去 —— 不能丢,丢了界面会永远停在「排队中…」。
        this.pending = [];
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
            this.#dispatch(m);
        };
        return true;
    }

    /* 点一次「编译」= 重扫一遍池子。断线之后也走这条,所以「连接已断开」
     * 之后直接再点编译就能换个后端,不用刷新页面。 */
    async submit(source) {
        if (!(await this.connect())) return;
        // 回包之前按"排队中"显示,免得点了没反应。
        this.handlers.onStatus("排队中…", "pending");
        this.#send({ type: "submit", source, target: "beyond-server" });
    }

    cancel() { this.#send({ type: "cancel" }); }

    #send(o) {
        // readyState 不是 1 就先入队 —— 首连握手和断线重连两条路径都靠它,
        // 否则消息静默消失,界面卡在「排队中…」。
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
            case "failed":
                this.handlers.onFailed(m);
                break;
            case "error":
                this.handlers.onError(m);
                break;
            default:
                // 认不出来的消息不忽略 —— 记一行。将来加 lsp 消息时,
                // 老前端就是靠这一行走过去的。
                this.handlers.onLog({ stream: "stderr", text: `未知消息 ${m.type}\n` });
        }
    }
}

function init() {
    const root = document.querySelector(".play");
    if (!root) return;

    const $ = (id) => root.querySelector(`#${id}`);
    const editor = new PlayEditor($("play-editor"));
    const status = $("play-status");
    const log = $("play-log");
    const runBtn = $("play-run");
    const cancelBtn = $("play-cancel");
    const dlBtn = $("play-download");
    const sampleSel = $("play-sample");
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
        editor.setEnabled(!busy);
    };

    // 下载地址在收到 finished 时才算得出来:后端给的是**服务端相对**
    // 路径(`/artifact/xxx`),而这一页在 GitHub Pages 上,直接用相对
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

    const client = new PlayClient(null, {
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
        },
    });

    runBtn.addEventListener("click", () => {
        log.textContent = "";
        dlBtn.hidden = true;
        downloadUrl = null;
        client.submit(editor.getText());
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

    client.connect();
}

if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
} else {
    init();
}