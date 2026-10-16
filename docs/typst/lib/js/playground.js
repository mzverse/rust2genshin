/* 摆弄页。
 *
 * 分成两个对象是刻意的:
 *   - PlayEditor  只认「取文本 / 显示文本」几个方法,将来换 CodeMirror
 *                 或加 rust-analyzer 的补全,只动它。
 *   - PlayClient  只认「连上 / 发 submit / 收消息」,不知道编辑器存在。
 * 两者在 PlaygroundApp 里通过那几个回调接起来。
 *
 * 默认后端是 ws://127.0.0.1:8787/ws —— 本机跑 `cargo run -p
 * rust2genshin-playground` 就能直接用。地址存在 localStorage 里。
 */

const DEFAULT_BACKEND = "ws://127.0.0.1:8787/ws";
const LS_KEY = "playground.backend";

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
        // 还没连上时想发的消息。首连握手 / 断线重连期间点「编译」都会
        // 落到这里,onopen 一冲就发出去 —— 不能丢,丢了界面会永远停在
        // 「排队中…」而没有任何解释。
        this.pending = [];
    }

    connect() {
        if (this.ws && this.ws.readyState <= 1) return;
        this.handlers.onStatus("连接中…", "pending");
        const ws = new WebSocket(this.url);
        this.ws = ws;
        ws.onopen = () => {
            this.handlers.onStatus("已连接", "ok");
            // 冲刷握手中/重连期间攒下的消息。
            const queued = this.pending;
            this.pending = [];
            for (const o of queued) this.#send(o);
        };
        ws.onclose = () => {
            // 陈旧 socket 守卫:重连后 this.ws 已指向新连接,旧 socket 的
            // close 随后到达时不能拿它覆盖新连接刚设的状态,也不能重置按钮态。
            if (ws !== this.ws) return;
            // 断连就把待发队列清掉,**不**在重连后重放。「编译」是有副作用的
            // 动作,在用户不知情时静默占一个排队位,比丢一次没送达的点击更坏;
            // 用户看到「连接已断开」会自己重试。
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
    }

    submit(source) {
        this.connect();
        // 回包之前先按"排队中"显示,免得点了没反应。
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

class PlaygroundApp extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: "open" });
        const tpl = document.getElementById("playground");
        this.shadowRoot.appendChild(tpl.content.cloneNode(true));
    }

    connectedCallback() {
        const $ = (p) => this.shadowRoot.querySelector(`[part="${p}"]`);
        const editor = new PlayEditor($("editor"));
        const status = $("status");
        const log = $("log");
        const runBtn = $("run");
        const cancelBtn = $("cancel");
        const dlBtn = $("download");
        const backendInput = $("backend-input");
        backendInput.value = localStorage.getItem(LS_KEY) || DEFAULT_BACKEND;

        const setStatus = (text, kind) => {
            status.textContent = text;
            // 状态色按宿主上的 data-status 选,不写在 part 元素自己身上:
            // CSS 的 ::part 是伪元素,后面接不了属性选择器,只能让属性选择器
            // 落在宿主 playground-app 上(card.css 的 &[type="info"]::part(icon)
            // 就是这个写法)。
            this.dataset.status = kind || "";
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

        const client = new PlayClient(backendInput.value, {
            onStatus: setStatus,
            onHello: (m) => {
                ready = !!m.image_commit;
                // 没烘好之前按钮是灰的 —— 后端也会拒,但让用户先点再被拒
                // 是白费一次往返。
                runBtn.disabled = !ready;
                const c = ready ? m.image_commit.slice(0, 7) : "尚无";
                setStatus(`就绪 · 镜像 ${c}`, ready ? "ok" : "pending");
                if (!ready) {
                    appendLog("stderr", "后端还在烘焙镜像，第一次可能要几分钟。\n");
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
        $("sample").addEventListener("click", async () => {
            try {
                const r = await fetch(`${httpOrigin(client.url)}/sample`);
                editor.setText(r.ok ? await r.text() : "// 取不到样例源码\n");
                if (!r.ok) return;
                // 样例来自工作区,镜像是某个 sha 烘的,两者可能不是同一个
                // commit(典型:lib/ 加了 API 但还没重烘焙)。这时样例可能
                // 编不过,提前说一句,别让人以为是自己的代码写坏了。
                const head = r.headers.get("x-playground-head");
                const baked = r.headers.get("x-playground-baked");
                if (head && baked && !baked.startsWith(head.slice(0, 7))) {
                    appendLog("stderr",
                        `注意:样例来自 ${head.slice(0, 7)},而镜像是 ${baked.slice(0, 7)} 烘的,可能编不过。\n`);
                }
            } catch {
                editor.setText("// 取不到样例源码,检查上面的后端地址\n");
            }
        });
        backendInput.addEventListener("change", () => {
            localStorage.setItem(LS_KEY, backendInput.value);
            window.location.reload();
        });

        client.connect();
    }
}

if (!customElements.get("playground-app")) {
    customElements.define("playground-app", PlaygroundApp);
}
