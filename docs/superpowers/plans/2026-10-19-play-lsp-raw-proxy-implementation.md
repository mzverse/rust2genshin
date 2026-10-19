# 摆弄页 LSP 原样转发迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把摆弄页的 LSP 通道改成「服务端只管会话与管道，其余消息原样双向转发，前端自己发请求、自己消费结果」。

**Architecture:** `ClientMsg::Lsp { message }` / `ServerMsg::Lsp { message }` 是唯一的 LSP 信封；服务端只拦截 `initialize`、`initialized`、`shutdown`/`exit`、`didOpen`/`didChange`，其余请求、通知、响应一律写进 RA 管道 / 转给浏览器。RA 会话仍然由池跨连接复用，交棒前由服务端补 `didClose`，因此浏览器的 `didOpen` 永远合法。

**Tech Stack:** Rust（rust-analyzer over `docker exec`、tokio、serde_json、axum WebSocket）、浏览器端 CodeMirror 6 + 原生 ES module。

**Spec:** `docs/superpowers/specs/2026-10-19-play-lsp-raw-proxy-design.md`

## Global Constraints

- `PROTOCOL_VERSION` 从 `2` 抬到 `3`（`playground/src/protocol.rs:11`）。
- LSP 报文只放在 `message` 字段里，服务端不重命名、不重组、不翻译任何 LSP 字段。
- 服务端只拦截清单里的消息：`initialize`、`initialized`、`shutdown`、`exit`、`textDocument/didOpen`、`textDocument/didChange`；其余全部原样转发。
- 关闭信号复用 LSP 的 `exit` 通知（服务端合成，`params.reason` 携带原因），不新增应用层消息。`shutdown` 是带 `id` 的请求，不能当关闭信号。
- 文档只有一个：`DOC_URI = "file:///opt/ra/ws/demo/src/lib.rs"`（`playground/src/lsp/session.rs:160`），不做多文件。
- 落盘不能省：`didOpen` / `didChange` 的全文必须先写进容器内 `demo/src/lib.rs`，再转发给 RA。
- 交棒后服务端不再发任何 LSP 请求；`Pending` 只在待命阶段（`initialize`、就绪探测、`ping`）使用。
- 不新增依赖；不引入 `@codemirror/lsp-client`。
- 文件操作一律用完整 Windows 绝对路径；提交时只 `git add` 计划里点名的文件，禁止 `git add .` / `git add -A`。
- 本计划不 push。每个任务末尾的 commit 只在用户允许提交后执行；若用户要求先验收，则跳过所有 commit 步骤，最后统一提交。
- 文档站是公开的：文档里只写 URL / 仓库相对路径，不写本机路径。
- 两个项目都非官方，文档与提交信息不得暗示与米哈游有关联。

## Review Focus

1. **未 `lsp_open` 就发 LSP 消息**：应回 `bad_request`（提示先发 `lsp_open`），不能静默丢弃或 panic。→ Task 4 的 `lsp_message_without_open_is_a_bad_request`。
2. **`didOpen` / `didChange` 拿不到全文**（缺 `text`、`contentChanges` 为空、带 `range` 的增量）：不能落盘半份内容再转发，应回 `bad_request`。→ Task 4 的 `document_text_requires_full_text`。
3. **待命阶段 RA 发请求而无人应答**：服务端必须替它回 `result: null`，否则 RA 挂起、会话永远不就绪。→ Task 4 的 `server_requests_are_answered_only_when_nobody_is_listening`。
4. **浏览器 id 落进内部区间**：交棒后 `Pending` 为空，`id >= INTERNAL_ID_BASE` 的响应也必须原样转回，不能被当成内部响应吞掉。→ Task 2 的 `unknown_responses_are_forwarded_not_swallowed`。
5. **断线重连后的会话复用**：旧浏览器必须已 `didClose`，新会话的 `didOpen` 才合法；会话应回池复用而不是新建。→ Task 4 的 ignored 集成测试 + Task 7 手工重连检查。

---

### Task 1: 协议信封与版本号

**Files:**
- Modify: `C:\projects\rust2genshin\playground\src\protocol.rs`
- Test: `C:\projects\rust2genshin\playground\src\protocol.rs`（同文件 `mod tests`）

**Interfaces:**
- Consumes: 无。
- Produces: `ClientMsg::Lsp { message: serde_json::Value }`、`ServerMsg::Lsp { message: serde_json::Value }`、`PROTOCOL_VERSION = 3`。旧变体与 `LspReady { file }` 暂时保留，Task 4 再删。关闭信号不新增变体，Task 4 用合成的 `exit` 通知实现。

- [ ] **Step 1: 先写失败测试**

在 `protocol.rs` 的 `mod tests` 里追加（放在 `lsp_open_roundtrips` 之后）：

```rust
    #[test]
    fn client_lsp_envelope_roundtrips_verbatim() {
        // LSP 报文必须原样躺在 `message` 里：服务端不重命名字段、
        // 不重组结构，否则前端拿到的就不是它发出去的那条。
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "textDocument/completion",
            "params": { "position": { "line": 3, "character": 12 } }
        });
        let m = ClientMsg::Lsp { message: message.clone() };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "lsp", "message": message })
        );
    }

    #[test]
    fn server_lsp_envelope_roundtrips_verbatim() {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": "file:///x", "diagnostics": [] }
        });
        let m = ServerMsg::Lsp { message: message.clone() };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "lsp", "message": message })
        );
    }

    #[test]
    fn protocol_version_is_3() {
        assert_eq!(PROTOCOL_VERSION, 3);
    }
```

同时把 `hello_json_shape` 里的 `"protocol": 2` 改成 `"protocol": 3`，并删除旧的 `protocol_version_is_2`。

- [ ] **Step 2: 跑测试确认失败**

```shell
cargo test -p rust2genshin-playground protocol::tests
```

Expected: 编译失败，`no variant or associated item named Lsp`。

- [ ] **Step 3: 实现**

`PROTOCOL_VERSION` 改成 3：

```rust
pub const PROTOCOL_VERSION: u32 = 3;
```

`ClientMsg` 增加变体（保留 `LspOpen`，`LspChange` / `LspCompletion` 留到 Task 4 删）：

```rust
    /// 一条原样的 LSP 报文。`message` 在语义上就是 JSON-RPC 报文本身，
    /// 服务端只解析 `method` / `id` 用于握手与落盘拦截，其余字段不碰。
    Lsp { message: serde_json::Value },
```

`ServerMsg` 增加一个变体：

```rust
    /// 一条原样的 LSP 报文（RA → 浏览器）。管道断开时服务端也用它
    /// 发一条合成的 `exit` 通知，前端据此视为传输终止。
    Lsp { message: serde_json::Value },
```

- [ ] **Step 4: 跑测试确认通过**

```shell
cargo test -p rust2genshin-playground protocol::tests
```

Expected: PASS（旧 LSP 测试此时仍在，应全绿）。

- [ ] **Step 5: Commit**

```shell
git add playground/src/protocol.rs
git commit -m "feat(play): 协议增加原样 LSP 信封，版本抬到 3

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: Session 的原样转发层（叠加，不删旧路径）

**Files:**
- Modify: `C:\projects\rust2genshin\playground\src\lsp\session.rs`
- Modify: `C:\projects\rust2genshin\playground\src\server\ws.rs`（只加一个空分支，保持编译）
- Test: `C:\projects\rust2genshin\playground\src\lsp\session.rs`（同文件 `mod tests`）

**Interfaces:**
- Consumes: `serde_json::Value`；`RaProcess::send` / `recv`。
- Produces:
  - `Event::Message(Value)`；
  - `Session::send_raw(&self, &Value) -> Result<(), LspError>`；
  - `Session::request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, LspError>`（返回**完整响应**，含 `result` / `error`）；
  - `Session::did_close(&self) -> Result<(), LspError>`（幂等）；
  - `Session::initialize_result(&self) -> Option<Value>`；
  - `Session::did_open` 现在会把内部 `doc_open` 置 true。

- [ ] **Step 1: 先写失败测试**

在 `session.rs` 的 `mod tests` 末尾追加：

```rust
    #[tokio::test]
    async fn initialize_caches_the_result_for_replay() {
        // 浏览器拿不到真正的 `initialize`（RA 一个进程只能 initialize
        // 一次），服务端必须缓存待命阶段那份响应，之后按浏览器 id 回放。
        let (fake, session) = setup().await;
        let tx = fake.sender();
        let feeder = tokio::spawn({
            let fake = Arc::clone(&fake);
            async move {
                let id = fake
                    .wait_until_written("\"method\":\"initialize\"")
                    .await
                    .expect("initialize 该带 id");
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "capabilities": {
                        "semanticTokensProvider": {
                            "legend": { "tokenTypes": ["keyword", "comment"] }
                        }
                    }},
                });
                tx.send(frame::encode(&resp.to_string()))
                    .expect("UnboundedSender 不该 send 失败");
            }
        });
        session.initialize().await.expect("initialize 该成功");
        feeder.await.expect("feeder 不该 panic");

        let cached = session
            .initialize_result()
            .expect("初始化之后必须能取到缓存的 result");
        assert_eq!(
            cached["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"][0],
            "keyword"
        );
    }

    #[tokio::test]
    async fn did_open_and_close_track_the_document() {
        let (fake, session) = setup().await;
        session.did_open("fn main() {}").await.expect("didOpen 该成功");
        session.did_close().await.expect("didClose 该成功");
        // 幂等：没有打开的文档就不再发第二条 didClose。
        session.did_close().await.expect("第二次 didClose 也不该报错");

        let bodies = fake.written_bodies();
        let count = |m: &str| bodies.iter().filter(|v| v["method"] == m).count();
        assert_eq!(count("textDocument/didOpen"), 1);
        assert_eq!(count("textDocument/didClose"), 1);

        // 从没 open 过的会话，didClose 是 no-op。
        let (fresh, session) = setup().await;
        session.did_close().await.expect("空会话 didClose 不该报错");
        assert!(fresh.written_bodies().is_empty());
    }

    #[tokio::test]
    async fn forwarded_messages_reach_ra_verbatim() {
        let (fake, session) = setup().await;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": DOC_URI },
                "position": { "line": 1, "character": 2 },
            },
        });
        session.send_raw(&msg).await.expect("转发该成功");
        assert_eq!(fake.written_bodies(), vec![msg]);
    }

    #[tokio::test]
    async fn ra_messages_are_forwarded_verbatim() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "window/logMessage",
            "params": { "type": 3, "message": "hi" },
        });
        fake.sender()
            .send(frame::encode(&msg.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        match next_event(&mut events).await {
            Event::Message(v) => assert_eq!(v, msg),
        }
    }

    #[tokio::test]
    async fn unknown_responses_are_forwarded_not_swallowed() {
        // 交棒后服务端不再发请求，Pending 为空；浏览器的 id 哪怕落进
        // 内部保留区间，响应也必须原样回去，不能被当成内部响应吃掉。
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": INTERNAL_ID_BASE + 1,
            "result": { "ok": true },
        });
        fake.sender()
            .send(frame::encode(&resp.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        match next_event(&mut events).await {
            Event::Message(v) => assert_eq!(v, resp),
        }
    }
```

- [ ] **Step 2: 跑测试确认失败**

```shell
cargo test -p rust2genshin-playground lsp::session::tests
```

Expected: 编译失败，`no variant named Message` / `no method named send_raw`。

- [ ] **Step 3: 实现**

在 `Event` 里加变体（保留旧三个，Task 4 删）：

```rust
    /// RA 发来的任意一帧，原样转发给浏览器。
    Message(Value),
```

`Session` 增加字段：

```rust
    /// 待命阶段 `initialize` 的 `result`，交棒后按浏览器的 id 回放。
    initialize_result: Arc<Mutex<Option<Value>>>,
    /// 当前文档在 RA 侧是否处于打开状态。交棒 / 归还前据此补 `didClose`。
    doc_open: AtomicBool,
```

`Session::new` 初始化它们，并把新字段传给 reader 时**不需要**（reader 不碰它们）。

`initialize` 改为复用 `request`，并缓存结果：

```rust
    pub async fn initialize(&self) -> Result<(), LspError> {
        let params = json!({
            "processId": Value::Null,
            "rootUri": format!("file://{RA_WORKDIR}"),
            "capabilities": {
                "textDocument": {
                    "synchronization": { "didSave": false },
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false,
                            "labelDetailsSupport": true
                        }
                    },
                    "diagnostic": {},
                    "semanticTokens": {
                        "requests": { "full": true },
                        "formats": ["relative"],
                        "tokenTypes": TOKEN_TYPES,
                        "tokenModifiers": [],
                    }
                }
            }
        });
        let resp = self.request("initialize", params, INIT_TIMEOUT).await?;
        if let Some(err) = resp.get("error") {
            return Err(LspError::BadResponse(err.to_string()));
        }
        let result = resp.get("result").cloned().unwrap_or(Value::Null);
        *self
            .initialize_result
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic") = Some(result);

        self.send_json(&json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {},
        }))
        .await
    }

    /// 待命阶段缓存的 `initialize` 结果。交棒后服务端用它按浏览器的 id 回放。
    pub fn initialize_result(&self) -> Option<Value> {
        self.initialize_result
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .clone()
    }
```

`did_open` / `did_change` 记住文档已打开（旧的 token/诊断请求暂时保留，Task 4 删）：

```rust
    pub async fn did_open(&self, text: &str) -> Result<(), LspError> {
        self.version.store(1, Ordering::SeqCst);
        self.doc_open.store(true, Ordering::SeqCst);
        // ...原来的 json! 与 request_tokens/request_diagnostics 保持不变...
    }

    pub async fn did_change(&self, text: &str) -> Result<(), LspError> {
        self.doc_open.store(true, Ordering::SeqCst);
        // ...原实现不变...
    }
```

新增 `did_close` / `send_raw`：

```rust
    /// 关闭当前文档。幂等：没有打开的文档直接返回。
    ///
    /// 交棒（`LspService::acquire`）与归还（`ws` 的收尾）都要调：
    /// 池里的会话跨连接复用，只有把上一份 overlay 关掉，新浏览器的
    /// `didOpen` 才是合法的。
    pub async fn did_close(&self) -> Result<(), LspError> {
        if !self.doc_open.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        self.send_json(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": DOC_URI } }
        }))
        .await
    }

    /// 原样转发一条 LSP 报文，并维护文档开关状态。
    pub async fn send_raw(&self, msg: &Value) -> Result<(), LspError> {
        match msg.get("method").and_then(Value::as_str) {
            Some("textDocument/didOpen") => self.doc_open.store(true, Ordering::SeqCst),
            Some("textDocument/didClose") => self.doc_open.store(false, Ordering::SeqCst),
            _ => {}
        }
        self.send_json(msg).await
    }
```

新增通用 `request`，`ping` 改为调用它：

```rust
    /// 发一个服务端自己的请求并等响应（完整响应，`error` 也算有响应）。
    /// 只在待命阶段使用：`initialize`、就绪探测、`ping`。
    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, LspError> {
        let id = self.next_id();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .insert(id, Pending::Response(resp_tx));
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        if let Err(e) = self.send_json(&req).await {
            self.unregister(id);
            return Err(e);
        }
        match tokio::time::timeout(timeout, resp_rx).await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(_)) => Err(LspError::Closed),
            Err(_) => {
                self.unregister(id);
                Err(LspError::Timeout)
            }
        }
    }

    pub async fn ping(&self, timeout: Duration) -> Result<(), LspError> {
        self.request("r2g/ping", Value::Null, timeout).await.map(|_| ())
    }
```

`dispatch` 增加原样转发（旧翻译分支全部保留）：

```rust
async fn dispatch(
    v: Value,
    proc: &RaProcess,
    tx: &broadcast::Sender<Event>,
    pending: &Mutex<HashMap<u64, Pending>>,
    next_id: &AtomicU64,
    legend: &Mutex<TokenLegend>,
    last_diagnostics: &Mutex<Vec<Diagnostic>>,
) {
    if let Some(method) = v.get("method").and_then(Value::as_str) {
        // ...旧分支保持不变（publishDiagnostics / refresh / 回 null）...

        // 新增：不管认不认识，都原样转发一份。旧 ws 会忽略它，
        // Task 4 切过去之后它就是唯一的上行通道。
        let _ = tx.send(Event::Message(v));
        return;
    }

    let Some(id) = v.get("id").and_then(Value::as_u64) else {
        let _ = tx.send(Event::Message(v));
        return;
    };
    let pending = pending
        .lock()
        .expect("Mutex 不该中毒:持锁段不 panic")
        .remove(&id);
    let Some(pending) = pending else {
        // 不是服务端发出的请求 —— 可能是浏览器自己的响应，原样回传。
        let _ = tx.send(Event::Message(v));
        return;
    };
    // ...原来的 match pending 保持不变...
}
```

`ws.rs` 的旧转发任务里加一个空分支，保证 `Event` 穷尽：

```rust
                                            Ok(Event::Message(_)) => continue,
```

- [ ] **Step 4: 跑测试确认通过**

```shell
cargo test -p rust2genshin-playground lsp::session::tests
```

Expected: PASS（新增用例与旧的 legend/诊断/token 用例同时通过）。

- [ ] **Step 5: Commit**

```shell
git add playground/src/lsp/session.rs playground/src/server/ws.rs
git commit -m "feat(play): Session 增加 LSP 原样转发与 initialize 缓存

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: 就绪判据换成诊断响应，交棒前补 didClose

**Files:**
- Modify: `C:\projects\rust2genshin\playground\src\lsp\service.rs`

**Interfaces:**
- Consumes: Task 2 的 `Session::request`、`Session::did_close`、`Session::initialize_result`（本任务只用前两个）、`DOC_URI`。
- Produces: `LspService::acquire` 在返回前保证会话没有打开的文档；`SessionMaker::make` 在返回前保证 `Pending` 为空。

- [ ] **Step 1: 改写就绪探测**

删掉 `use tokio::sync::broadcast;`、`use crate::lsp::session::Event;`，改为：

```rust
use serde_json::json;

use crate::lsp::session::{DOC_URI, Session};
```

把 `make()` 第 4、5 步替换为（`initialize` 与 `did_open(PLACEHOLDER)` 保留）：

```rust
        let session = Arc::new(Session::new(Arc::clone(&proc)));

        if let Err(e) = session.initialize().await {
            tracing::warn!("RA initialize 失败:{e}");
            teardown(&name, &session).await;
            return None;
        }
        if let Err(e) = session.did_open(PLACEHOLDER).await {
            tracing::warn!("RA didOpen 失败:{e}");
            teardown(&name, &session).await;
            return None;
        }

        // 就绪判据：服务端用自己的 id 拉一次诊断，拿到响应即说明 crate
        // graph 已加载。这里只等「有响应」，不看诊断内容；请求在交棒前
        // 一定 await 完，交棒后 Pending 必为空。
        if let Err(e) = session
            .request(
                "textDocument/diagnostic",
                json!({ "textDocument": { "uri": DOC_URI } }),
                self.cfg.lsp_boot_timeout,
            )
            .await
        {
            tracing::warn!("RA 就绪探测失败,丢弃这次会话:{e}");
            teardown(&name, &session).await;
            return None;
        }
```

同时更新文件头第 4、5 步的说明（先 `initialize` → `didOpen(占位)` → 用诊断请求判就绪）。

- [ ] **Step 2: 交棒前补 didClose**

```rust
    /// 取一个会话。池里有活的直接给，没有就现造。
    ///
    /// 返回前保证**没有打开的文档**：池里的会话可能还带着上一份占位或
    /// 上一个浏览器的 overlay。补一条 `didClose` 之后，浏览器的 `didOpen`
    /// 永远合法，也不需要把 didOpen 改写成 didChange。
    pub async fn acquire(&self) -> Result<Acquire, PoolError> {
        let acq = self.pool.acquire().await?;
        if let Err(e) = acq.session().did_close().await {
            tracing::warn!("交棒前 didClose 失败,丢弃这个会话:{e}");
            acq.release(false).await;
            return Err(PoolError::Create);
        }
        Ok(acq)
    }
```

- [ ] **Step 3: 编译并跑现有测试**

```shell
cargo test -p rust2genshin-playground lsp::service lsp::pool lsp::session
```

Expected: PASS。就绪探测的真路径只有 docker 能验证，放到 Task 4 的 ignored 集成测试与 Task 7 的手工检查。

- [ ] **Step 4: Commit**

```rust
git add playground/src/lsp/service.rs
git commit -m "refactor(play): RA 就绪改用诊断响应，交棒前补 didClose

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: 切到原样转发，删除旧翻译层

这是原子切换：`session.rs`、`ws.rs`、`protocol.rs` 必须同一提交，否则中间状态编译不过。

**Files:**
- Modify: `C:\projects\rust2genshin\playground\src\lsp\session.rs`
- Modify: `C:\projects\rust2genshin\playground\src\server\ws.rs`
- Modify: `C:\projects\rust2genshin\playground\src\protocol.rs`
- Test: 上述三个文件的 `mod tests`

**Interfaces:**
- Consumes: Task 1 的 `ClientMsg::Lsp` / `ServerMsg::Lsp`；Task 2 的 `Event::Message` / `send_raw` / `did_close` / `initialize_result`。
- Produces:
  - `ServerMsg::LspReady { file, uri }`；
  - `Session::did_open(&str)` 只发 `didOpen`（不再顺带拉 token / 诊断）；
  - `ws::route_lsp_method(Option<&str>) -> LspRoute`；
  - `ws::document_text(&Value) -> Option<&str>`；
  - `ws::initialize_reply(&Value, &Value) -> Value`。

- [ ] **Step 1: 先写失败测试**

**(a) `session.rs`**

删除以下旧测试（它们测的是翻译层）：
`initialize_then_did_open_then_did_change`、`did_change_sends_full_text_not_range`、`publish_diagnostics_becomes_an_event`、`diagnostics_are_filtered_by_uri_and_empty_clears`、`did_change_pulls_diagnostics`、`pulled_diagnostics_become_an_event`、`unchanged_diagnostics_replay_the_cached_set`、`diagnostic_refresh_notification_pulls_again`、`did_open_requests_semantic_tokens`、`semantic_tokens_pass_through_untouched`、`malformed_token_data_emits_no_event`、`legend_*`（四条）、`tokens_are_remapped_through_the_legend_from_initialize`、`server_requests_are_answered_and_refresh_refetches_tokens`、`completion_request_uses_the_callers_id`。

保留 `reader_exit_wakes_pending_initialize`、`session_ping_goes_through_the_pending_map`、Task 2 新增的五条，并追加：

```rust
    /// 待命阶段没人订阅：RA 的请求必须由服务端替它回 null，否则 RA
    /// 一直等，会话永远不就绪。
    #[tokio::test]
    async fn server_requests_are_answered_only_when_nobody_is_listening() {
        let (fake, session) = setup().await;
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "workspace/configuration",
            "params": { "items": [] },
        });
        fake.sender()
            .send(frame::encode(&req.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        let _ = fake.wait_until_written("\"id\":99").await;
        let bodies = fake.written_bodies();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0]["result"], Value::Null);

        // 有前端时原样转发，服务端不抢答 —— 抢答会和浏览器的响应
        // 撞成两条同 id 的响应。
        let mut events = session.subscribe();
        let req2 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "client/registerCapability",
            "params": {},
        });
        fake.sender()
            .send(frame::encode(&req2.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        match next_event(&mut events).await {
            Event::Message(v) => assert_eq!(v, req2),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !fake.written_bodies().iter().any(|v| v["id"] == 100),
            "有前端时服务端不该替它回答"
        );
    }
```

**(b) `protocol.rs`**

删除 `ClientMsg::LspChange` / `ClientMsg::LspCompletion` 与 `ServerMsg::LspTokens` / `LspDiagnostics` / `LspCompletion` 及结构体 `LspDiagnosticItem` / `CompletionLabelDetails` / `CompletionItem`。删除对应旧测试。追加：

```rust
    #[test]
    fn lsp_ready_carries_the_document_uri() {
        // 前端要拿它填 textDocument.uri；只给相对路径不够。
        let m = ServerMsg::LspReady {
            file: SOURCE_FILE.into(),
            uri: "file:///opt/ra/ws/demo/src/lib.rs".into(),
        };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "type": "lsp_ready",
                "file": "demo/src/lib.rs",
                "uri": "file:///opt/ra/ws/demo/src/lib.rs",
            })
        );
    }
```

**(c) `ws.rs`**

在 `mod tests` 的纯函数区追加：

```rust
    #[test]
    fn lsp_routes_only_the_handshake_and_text_sync() {
        assert_eq!(route_lsp_method(Some("initialize")), LspRoute::Initialize);
        for m in ["initialized", "shutdown", "exit"] {
            assert_eq!(route_lsp_method(Some(m)), LspRoute::Swallow, "{m}");
        }
        for m in ["textDocument/didOpen", "textDocument/didChange"] {
            assert_eq!(route_lsp_method(Some(m)), LspRoute::Store, "{m}");
        }
        for m in [
            "textDocument/definition",
            "textDocument/completion",
            "textDocument/diagnostic",
            "textDocument/semanticTokens/full",
        ] {
            assert_eq!(route_lsp_method(Some(m)), LspRoute::Forward, "{m}");
        }
        // 没有 method 的帧是浏览器对 RA 请求的响应，也必须转发。
        assert_eq!(route_lsp_method(None), LspRoute::Forward);
    }

    #[test]
    fn document_text_requires_full_text() {
        let open = serde_json::json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "text": "fn main() {}" } },
        });
        assert_eq!(document_text(&open), Some("fn main() {}"));

        let change = serde_json::json!({
            "method": "textDocument/didChange",
            "params": { "contentChanges": [{ "text": "fn x() {}" }] },
        });
        assert_eq!(document_text(&change), Some("fn x() {}"));

        // 缺文本 / 空 changes / 带 range 的增量：都拿不到全文，不能落盘。
        assert_eq!(
            document_text(&serde_json::json!({
                "method": "textDocument/didOpen",
                "params": {},
            })),
            None
        );
        assert_eq!(
            document_text(&serde_json::json!({
                "method": "textDocument/didChange",
                "params": { "contentChanges": [] },
            })),
            None
        );
        assert_eq!(
            document_text(&serde_json::json!({
                "method": "textDocument/didChange",
                "params": { "contentChanges": [{ "range": {} }] },
            })),
            None
        );
    }

    #[test]
    fn initialize_reply_uses_the_browser_id_and_the_cached_result() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 17,
            "method": "initialize",
            "params": {},
        });
        let cached = serde_json::json!({
            "capabilities": {
                "semanticTokensProvider": {
                    "legend": { "tokenTypes": ["keyword"] }
                }
            }
        });
        let reply = initialize_reply(&request, &cached);
        assert_eq!(reply["jsonrpc"], "2.0");
        assert_eq!(reply["id"], 17, "必须回浏览器的 id，不是内部 id");
        assert_eq!(reply["result"], cached);
    }

    #[test]
    fn closed_frame_is_an_exit_notification() {
        // 关闭信号复用 LSP 的 exit 通知：无 id（不是请求，不需要应答），
        // 原因放 params.reason。用 shutdown 的话前端还得回一条 null。
        let frame = closed_frame("分析器已退出");
        assert_eq!(frame["jsonrpc"], "2.0");
        assert_eq!(frame["method"], "exit");
        assert!(frame.get("id").is_none(), "exit 必须是通知");
        assert_eq!(frame["params"]["reason"], "分析器已退出");
    }

    #[tokio::test]
    async fn lsp_message_without_open_is_a_bad_request() {
        let (_d, url, _st) =
            spawn_with_lsp(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(
            &mut ws,
            ClientMsg::Lsp {
                message: serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "textDocument/completion",
                    "params": {},
                }),
            },
        )
        .await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, message } => {
                assert_eq!(code, ErrorCode::BadRequest);
                assert!(message.contains("lsp_open"), "{message}");
            }
            other => panic!("期望 error,得到 {other:?}"),
        }
    }
```

把旧的 `lsp_change_without_open_is_a_bad_request` 删除（`ClientMsg::LspChange` 已经不存在）。

- [ ] **Step 2: 跑测试确认失败**

```shell
cargo test -p rust2genshin-playground server::ws::tests
```

Expected: 编译失败，`LspRoute not found` / `document_text not found`。

- [ ] **Step 3: 实现——`protocol.rs`**

`LspReady` 加 `uri`：

```rust
    LspReady { file: String, uri: String },
```

删掉旧 LSP 变体与三个结构体（`LspDiagnosticItem` / `CompletionLabelDetails` / `CompletionItem`）。

- [ ] **Step 4: 实现——`session.rs`**

删除：`TokenLegend`、`CompletionLabelDetails`、`CompletionItem`、`Diagnostic`、`Pending::{Tokens, Diagnostics, Completion}`、`legend`、`last_diagnostics`、`version`、`LspError::BadRequestId`、`did_change`、`request_completion`、`request_tokens`、`request_diagnostics`、`request_tokens_impl`、`request_diagnostics_impl`、`emit_diagnostics`、`parse_diagnostic_result`、`diagnostic_from`、`parse_completion_items`。

`Event` 只剩：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// RA 发来的一帧，原样转发给浏览器。
    Message(Value),
}
```

`Pending` 只剩：

```rust
enum Pending {
    Response(oneshot::Sender<Value>),
}
```

`Session::did_open` 只发 `didOpen`：

```rust
    /// 打开文档（全文）。待命阶段灌占位内容用；交棒后浏览器自己的
    /// `didOpen` 由 ws 原样转发。
    pub async fn did_open(&self, text: &str) -> Result<(), LspError> {
        self.doc_open.store(true, Ordering::SeqCst);
        self.send_json(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": DOC_URI,
                    "languageId": "rust",
                    "version": 1,
                    "text": text,
                }
            }
        }))
        .await
    }
```

`reader_loop` 去掉 `next_id` / `legend` / `last_diagnostics` 三个参数（它们只被旧 `dispatch` 用），`dispatch` 调用同步缩短；`Session::new` 里给 reader 的 spawn 也要跟着改。

`dispatch` 完整替换为：

```rust
/// 把 RA 的一帧分到 pending 表或转发端。
///
/// - 带 `method` 的是通知或 RA 发来的请求。请求（带非 null `id`）在
///   **没有订阅者**时由服务端替它回 `null`（待命 / 交棒间隙），否则
///   原样转给前端、由前端回答；通知一律转发。
/// - 没有 `method` 的是响应：命中 `Pending` 的是服务端自己的请求，
///   交给 oneshot；没命中的是浏览器请求的响应，原样转发。
async fn dispatch(
    v: Value,
    proc: &RaProcess,
    tx: &broadcast::Sender<Event>,
    pending: &Mutex<HashMap<u64, Pending>>,
) {
    if v.get("method").and_then(Value::as_str).is_some() {
        if let Some(id) = v.get("id").filter(|id| !id.is_null()) {
            if tx.receiver_count() == 0 {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id.clone(),
                    "result": Value::Null,
                });
                if let Err(e) = proc.send(&resp.to_string()).await {
                    tracing::warn!("回复 RA 的服务端请求失败:{e}");
                }
            }
        }
        let _ = tx.send(Event::Message(v));
        return;
    }

    let Some(id) = v.get("id").and_then(Value::as_u64) else {
        // 既无 method 也无数字 id 的怪帧：转发出去，前端会忽略。
        let _ = tx.send(Event::Message(v));
        return;
    };
    let pending = pending
        .lock()
        .expect("Mutex 不该中毒:持锁段不 panic")
        .remove(&id);
    match pending {
        Some(Pending::Response(resp_tx)) => {
            let _ = resp_tx.send(v);
        }
        None => {
            let _ = tx.send(Event::Message(v));
        }
    }
}
```

更新文件头注释：不再是「翻译层」，而是「拆帧 → 配对服务端自己的响应 → 其余原样转发」。

- [ ] **Step 5: 实现——`ws.rs`**

导入改为：

```rust
use serde_json::{Value, json};

use crate::{
    docker::Stream,
    job::JobMeta,
    lsp::{
        pool::Acquire,
        session::{DOC_URI, Event},
    },
    protocol::{ClientMsg, ErrorCode, MAX_SOURCE_BYTES, PROTOCOL_VERSION, SOURCE_FILE, ServerMsg},
    runner::{JobSpec, RunnerError},
    server::{AppState, client_key_of, origin_allowed},
};
```

`LspOpen` 分支里把事件翻译任务换成原样中继（`broadcast` 仍要用于 `RecvError`）：

```rust
                                let mut events = acq.session().subscribe();
                                let ftx = tx.clone();
                                let forwarding = tokio::spawn(async move {
                                    loop {
                                        match events.recv().await {
                                            Ok(Event::Message(message)) => {
                                                if ftx
                                                    .send(Outbound::from(ServerMsg::Lsp { message }))
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            // 慢消费者丢几条不值得断连。
                                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                            Err(broadcast::error::RecvError::Closed) => {
                                                let _ = ftx.send(Outbound::from(ServerMsg::Lsp {
                                                    message: closed_frame("分析器已退出"),
                                                }));
                                                break;
                                            }
                                        }
                                    }
                                });
                                lsp_conn = Some(LspConn { acq, forwarding });
                                if send(
                                    &mut sink,
                                    &ServerMsg::LspReady {
                                        file: SOURCE_FILE.to_string(),
                                        uri: DOC_URI.to_string(),
                                    },
                                )
                                .await
                                .is_err()
                                {
                                    break;
                                }
```

删除 `ClientMsg::LspChange` / `LspCompletion` 两个分支，换成：

```rust
                            Ok(ClientMsg::Lsp { message }) => {
                                // 不把 `&LspConn` 借到 await 对面去（见旧注释）。
                                let Some((container, session)) = lsp_conn.as_ref().map(|c| {
                                    (c.acq.container().to_string(), Arc::clone(c.acq.session()))
                                }) else {
                                    if send(&mut sink, &bad_request("先发 lsp_open")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                };

                                match route_lsp_method(message.get("method").and_then(Value::as_str)) {
                                    LspRoute::Initialize => {
                                        let Some(cached) = session.initialize_result() else {
                                            if send(
                                                &mut sink,
                                                &err(ErrorCode::Internal, "分析器还没初始化完"),
                                            )
                                            .await
                                            .is_err()
                                            {
                                                break;
                                            }
                                            continue;
                                        };
                                        let reply = initialize_reply(&message, &cached);
                                        if send(
                                            &mut sink,
                                            &ServerMsg::Lsp { message: reply },
                                        )
                                        .await
                                        .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    // RA 在待命阶段已经收过 initialized；shutdown /
                                    // exit 会杀掉池里共享的进程。
                                    LspRoute::Swallow => {}
                                    LspRoute::Store => {
                                        let Some(svc) = state.lsp.clone() else {
                                            if send(
                                                &mut sink,
                                                &err(ErrorCode::Internal, "代码分析功能已关闭"),
                                            )
                                            .await
                                            .is_err()
                                            {
                                                break;
                                            }
                                            continue;
                                        };
                                        // 先写盘再转发：RA 可能先处理 didOpen、再从
                                        // 磁盘读 workspace 成员。
                                        let Some(text) = document_text(&message) else {
                                            if send(
                                                &mut sink,
                                                &bad_request("didOpen / didChange 必须带全文"),
                                            )
                                            .await
                                            .is_err()
                                            {
                                                break;
                                            }
                                            continue;
                                        };
                                        if let Err(e) = svc.write_source(&container, text).await {
                                            if !lsp_failed(
                                                &mut sink,
                                                &mut lsp_conn,
                                                format!("写源码失败:{e}"),
                                            )
                                            .await
                                            {
                                                break;
                                            }
                                            continue;
                                        }
                                        if let Err(e) = session.send_raw(&message).await {
                                            if !lsp_failed(
                                                &mut sink,
                                                &mut lsp_conn,
                                                format!("分析器断开了:{e}"),
                                            )
                                            .await
                                            {
                                                break;
                                            }
                                            continue;
                                        }
                                    }
                                    LspRoute::Forward => {
                                        if let Err(e) = session.send_raw(&message).await {
                                            if !lsp_failed(
                                                &mut sink,
                                                &mut lsp_conn,
                                                format!("分析器断开了:{e}"),
                                            )
                                            .await
                                            {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
```

在 `run_socket` 末尾的归还路径补 `didClose`：

```rust
    if let Some(c) = lsp_conn.take() {
        c.forwarding.abort();
        let session = Arc::clone(c.acq.session());
        // 浏览器没发过 didClose 的话替它补一条，池里的会话回到
        // 「没有打开文档」的状态。
        if let Err(e) = session.did_close().await {
            tracing::debug!("归还会话前 didClose 失败:{e}");
        }
        let alive = session.is_alive();
        c.acq.release(alive).await;
    }
```

新增辅助项（放在 `LspConn` 附近）：

```rust
/// 服务端对一条浏览器 LSP 消息的处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LspRoute {
    /// 用缓存的 `initialize` 结果按浏览器 id 回复，不转发。
    Initialize,
    /// 吞掉：RA 已经收过，或会杀死共享进程。
    Swallow,
    /// 落盘后原样转发。
    Store,
    /// 原样转发。
    Forward,
}

fn route_lsp_method(method: Option<&str>) -> LspRoute {
    match method {
        Some("initialize") => LspRoute::Initialize,
        Some("initialized" | "shutdown" | "exit") => LspRoute::Swallow,
        Some("textDocument/didOpen" | "textDocument/didChange") => LspRoute::Store,
        _ => LspRoute::Forward,
    }
}

/// `didOpen` 的 `textDocument.text`，或 `didChange` 的
/// `contentChanges[0].text`。拿不到全文返回 `None`：带 `range` 的增量
/// 不能用来落盘，宁可回 bad_request 也不写半份内容。
fn document_text(message: &Value) -> Option<&str> {
    let params = message.get("params")?;
    match message.get("method")?.as_str()? {
        "textDocument/didOpen" => params.get("textDocument")?.get("text")?.as_str(),
        "textDocument/didChange" => params.get("contentChanges")?.get(0)?.get("text")?.as_str(),
        _ => None,
    }
}

/// 用待命阶段缓存的 `result`，按**浏览器的 id** 回一条 initialize 应答。
fn initialize_reply(request: &Value, cached: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": request.get("id").cloned().unwrap_or(Value::Null),
        "result": cached,
    })
}

/// 服务端合成的关闭信号：一条 `exit` 通知。`exit` 在规范里是客户端→
/// 服务端，这里是本项目唯一的反向合成帧；前端收到即视为传输终止。
/// 不用 `shutdown`：它是带 id 的请求，会诱发一次没有意义的应答。
fn closed_frame(reason: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "exit",
        "params": { "reason": reason },
    })
}

/// 管道断了：发一次合成的 `exit`，销毁会话（不回池）。
/// 返回 false 表示这条 WebSocket 也该断（消息发不出去）。
async fn lsp_failed<S>(
    sink: &mut S,
    conn: &mut Option<LspConn>,
    why: String,
) -> bool
where
    S: futures_util::Sink<Message, Error = axum::Error> + Unpin,
{
    let sent = send(
        sink,
        &ServerMsg::Lsp {
            message: closed_frame(&why),
        },
    )
    .await
    .is_ok();
    if let Some(c) = conn.take() {
        c.forwarding.abort();
        c.acq.release(false).await;
    }
    sent
}
```

`LspConn` 的注释改成「一条连接持有的 RA 会话，以及把 RA 帧中继给前端的任务」。

- [ ] **Step 6: 改写 ignored 集成测试**

把 `lsp_session_serves_diagnostics_and_is_reused_after_disconnect` 整个替换为：

```rust
    /// 真起一个 RA 会话，走一遍原样转发，再验证断开后会话回池、能被
    /// 下一条连接复用。需要本机有 docker 与烘好的镜像。
    #[tokio::test]
    #[ignore = "需要本机有 docker 与烘好的镜像"]
    async fn lsp_session_forwards_raw_messages_and_is_reused_after_disconnect() {
        let mut cfg = crate::config::load().expect("读配置");
        let repo = Repo::new(cfg.repo_dir.clone());
        let Some(built) = crate::startup_plan(&cfg, &repo).await.serving else {
            return;
        };

        let d = tempfile::tempdir().unwrap();
        cfg.repo_dir = d.path().to_path_buf();
        cfg.data_dir = d.path().join("data");
        let cfg = Arc::new(cfg);
        let baker = Baker::new(
            FakeBuilder::new(vec![Ok(built)]),
            Arc::new(OkGate) as Arc<dyn DiskGate>,
        );
        baker.request(BakeRequest::default());
        baker.step().await;
        let svc = crate::lsp::service::LspService::new(Arc::clone(&cfg), Arc::clone(&baker));
        let state = Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker,
            jobs: Arc::new(JobStore::new(cfg.data_dir.join("tasks"))),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner: FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 }),
            started: std::time::Instant::now(),
            cfg,
            lsp: Some(Arc::clone(&svc)),
        });

        let first = svc.acquire().await.expect("镜像就绪该能起 RA");
        let container = first.container().to_string();
        assert!(!container.is_empty(), "真实会话要带容器名");
        first.release(true).await;

        let (url, _state) = serve(state).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, ClientMsg::LspOpen {}).await;
        let ready = read_until(&mut ws, |m| matches!(m, ServerMsg::LspReady { .. })).await;
        let uri = match ready {
            ServerMsg::LspReady { file, uri } => {
                assert_eq!(file, SOURCE_FILE);
                uri
            }
            other => panic!("期望 lsp_ready,得到 {other:?}"),
        };

        // initialize：服务端用缓存的 result 按浏览器 id 回放。
        send(
            &mut ws,
            ClientMsg::Lsp {
                message: serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": { "processId": null, "rootUri": null, "capabilities": {} },
                }),
            },
        )
        .await;
        read_until(&mut ws, |m| {
            matches!(m, ServerMsg::Lsp { message } if message["id"] == 1
                && message["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
                    .is_array())
        })
        .await;

        send(
            &mut ws,
            ClientMsg::Lsp {
                message: serde_json::json!({
                    "jsonrpc": "2.0", "method": "initialized", "params": {},
                }),
            },
        )
        .await;

        // didOpen 的全文必须落盘并原样进 RA。
        let text = "fn main() { let x: i32 = \"not an int\"; }";
        send(
            &mut ws,
            ClientMsg::Lsp {
                message: serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/didOpen",
                    "params": { "textDocument": {
                        "uri": uri, "languageId": "rust", "version": 1, "text": text,
                    }},
                }),
            },
        )
        .await;

        // 拉取式诊断：原样往返，RA 返回当前文件的诊断。
        send(
            &mut ws,
            ClientMsg::Lsp {
                message: serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "textDocument/diagnostic",
                    "params": { "textDocument": { "uri": uri } },
                }),
            },
        )
        .await;
        read_until(&mut ws, |m| {
            matches!(m, ServerMsg::Lsp { message }
                if message["id"] == 2
                    && message["result"]["items"].as_array().is_some_and(|a| !a.is_empty()))
        })
        .await;

        // 顺手证一下任意请求都能往返（不进入 UI）。
        send(
            &mut ws,
            ClientMsg::Lsp {
                message: serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "textDocument/definition",
                    "params": {
                        "textDocument": { "uri": uri },
                        "position": { "line": 0, "character": 4 },
                    },
                }),
            },
        )
        .await;
        read_until(&mut ws, |m| {
            matches!(m, ServerMsg::Lsp { message } if message["id"] == 3)
        })
        .await;

        drop(ws);
        tokio::time::sleep(Duration::from_secs(1)).await;
        let again = svc.acquire().await.expect("池里该留着刚才那一个");
        assert_eq!(
            again.container(),
            container,
            "断开后会话该回池复用,不能新建"
        );
        let alive = again.session().is_alive();
        again.release(alive).await;
    }
```

- [ ] **Step 7: 跑测试确认通过**

```shell
cargo test -p rust2genshin-playground
```

Expected: PASS，且不再有 `LspTokens` / `did_change` / `request_completion` 的编译引用。

- [ ] **Step 8: Commit**

```shell
git add playground/src/lsp/session.rs playground/src/server/ws.rs playground/src/protocol.rs
git commit -m "refactor(play): LSP 改为原样双向转发，删除服务端翻译层

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: 前端换成 JSON-RPC 层

**Files:**
- Modify: `C:\projects\rust2genshin\docs\typst\play\lsp-client.js`（整文件替换）

**Interfaces:**
- Consumes: `lsp_ready { uri }`、`lsp { message }`（关闭信号是其中合成的 `exit` 通知）。
- Produces:
  - `new LspClient({ view, send, onStatus })`；
  - `open()` → 发 `{type:"lsp_open"}`；
  - `onReady(uri)`；
  - `handleMessage(message)`；
  - `onClosed(message)`；
  - `onDocChanged(source)`；
  - `requestCompletion(line, column) -> Promise<items>`（items 形状与旧版一致：`label` / `kind` / `detail` / `label_details`）；
  - 导出 `kindToType`、`tokensField`、`diagnosticsField`、`decorationField`（`script.js` 的 import 不变）。

- [ ] **Step 1: 整文件替换为下列内容**

```javascript
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
```

- [ ] **Step 2: 静态检查**

```shell
node --check docs/typst/play/lsp-client.js
```

Expected: 无输出（Node 对 ES module 语法会接受 `import`，但若报 `Cannot use import statement`，说明本机 Node 按 CommonJS 解析；改用 `node --input-type=module --check < docs/typst/play/lsp-client.js`）。

- [ ] **Step 3: Commit**

```shell
git add docs/typst/play/lsp-client.js
git commit -m "refactor(play): 前端换成 JSON-RPC 层，tokens/诊断改为主动请求

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: script.js 接线

**Files:**
- Modify: `C:\projects\rust2genshin\docs\typst\play\script.js`

**Interfaces:**
- Consumes: Task 5 的 `lsp.onReady(uri)` / `lsp.handleMessage(message)`（合成的 `exit` 由 `LspClient` 自己消化）。
- Produces: `PlayClient` 的 `onLspReady` / `onLsp` 两个回调；`send` 不再把 `{type:"lsp"}` 入队。

- [ ] **Step 1: 改文件头注释**

把 8–14 行改成：

```javascript
 * 与 `lsp-client.js` 的分工:
 *   - script.js 管「WS 连上 / 发 submit / 收消息」+ UI 状态(target 选择器、
 *     日志面板、拖动条、选项卡、样例载入);
 *   - lsp-client.js 管「LSP 报文 ↔ CodeMirror」:自己发 initialize /
 *     didOpen / didChange / semanticTokens / 诊断 / 补全请求,收到 `lsp`
 *     信封后自己解析。script.js 只做透传。
```

- [ ] **Step 2: 改 `PlayClient`**

构造函数里 `this.onLsp = null;` 之后加：

```javascript
        // `lsp_ready { file, uri }` 的回调。和 onLsp 一样在 hello
        // 决定启用 LSP 后才设上。
        this.onLspReady = null;
```

`send` 的入队条件改成：

```javascript
        if (typeof o.type === "string"
            && !o.type.startsWith("lsp_") && o.type !== "lsp") {
            this.pending.push(o);
        }
```

`#dispatch` 的 LSP 分支改成：

```javascript
            case "lsp_ready":
                if (this.onLspReady) this.onLspReady(m);
                break;
            case "lsp":
                // RA 的原始报文，交给 LspClient 自己配对/消费。
                // 合成的 `exit` 也在里面，由 LspClient 自己收掉。
                if (this.onLsp) this.onLsp(m.message);
                break;
```

- [ ] **Step 3: 改 `onHello` 的接线**

```javascript
                if (lsp) lsp.dispose();
                lsp = new LspClient({
                    view,
                    send: (msg) => client.send(msg),
                    onStatus: (text) => setStatus(text, "ok"),
                });
                client.onLspReady = (m) => { if (lsp) lsp.onReady(m.uri); };
                client.onLsp = (message) => { if (lsp) lsp.handleMessage(message); };
                // 发 LspOpen —— 服务端在池里建一个 RA 会话，准备好了回 LspReady。
                lsp.open();
```

`else` 分支的清理改成：

```javascript
                if (lsp) { lsp.dispose(); lsp = null; }
                client.onLspReady = null;
                client.onLsp = null;
```

- [ ] **Step 4: 静态检查**

```shell
node --check docs/typst/play/script.js
```

Expected: 无输出。

- [ ] **Step 5: Commit**

```shell
git add docs/typst/play/script.js
git commit -m "feat(play): script.js 接线 lsp / lsp_ready

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 7: 文档、全量验证与手工检查

**Files:**
- Modify: `C:\projects\rust2genshin\docs\typst\arch\play.typ`
- 验证：整个 playground crate + 文档站 + 浏览器

**Interfaces:**
- Consumes: 前六个任务的成品。
- Produces: 与代码一致的站点叙述。

- [ ] **Step 1: 更新协议表**

把 83–96 行的 LSP 行替换为：

```typst
    [客户端 → 服务端], [`lsp_open`], [取一个 RA 会话；幂等。],
    [客户端 → 服务端], [`lsp`], [`{message}`；原样 JSON-RPC 报文。除握手与 `didOpen` / `didChange` 外全部转发。],
    [服务端 → 客户端], [`hello`], [连接后第一条：协议版本、镜像 commit、目标表、`lsp_supported`。],
    [服务端 → 客户端], [`queued`], [排队位次与当前队列长度。],
    [服务端 → 客户端], [`started`], [容器开始跑；带 `task` 与 `image_commit`。],
    [服务端 → 客户端], [`log`], [`{task, stream, text}`；stdout / stderr 分开。],
    [服务端 → 客户端], [`finished`], [成功；带产物字节数。产物*紧跟其后*以二进制帧发过来。],
    [服务端 → 客户端], [`failed`], [失败；`exit_code` 可空，`message` 是给用户看的。],
    [服务端 → 客户端], [`cancelled`], [`abort` 本身静默，必须显式回这一条。],
    [服务端 → 客户端], [`error`], [请求被拒：`not_ready` / `ip_busy` / `queue_full` / `bad_request` / `internal`。],
    [服务端 → 客户端], [`lsp`], [`{message}`；RA 的原始报文。管道断开时发一条合成的 `exit` 通知（`params.reason`），前端据此清高亮与诊断。],
    [服务端 → 客户端], [`lsp_ready`], [`{file, uri}`；会话已取得，前端拿 `uri` 填 `textDocument.uri`。],
```

- [ ] **Step 2: 更新 RA 会话一节**

把 166–183 行的内容替换为：

````typst
```text
浏览器编辑器
   │ lsp（除握手外原样转发）
   ▼
server::ws ──▶ LspService ──▶ Pool ──▶ Session ──▶ docker exec ──▶ rust-analyzer
   ▲               │             │          │
   └── lsp（含合成的 exit）──── Event ◀─────┘
```

*镜像里另烘一份分析工作区* `/opt/ra/ws`：只有 `demo` / `lib` / `lib-internal`，没有 `core`。`core` 依赖 `rustc_private`，RA 解析它要把整个编译器 sysroot 拖进来，冷启动从 11 秒涨到 84 秒。

*会话创建*：拿 serving tag → `docker run -d` 待命容器 → `docker exec -i rust-analyzer` → `initialize` → `didOpen(占位)` → 服务端用自己的 id 拉一次 `textDocument/diagnostic`，拿到响应即就绪。池里的会话跨连接复用，交棒前由服务端补 `didClose`，所以浏览器的 `didOpen` 永远合法。

*服务端只拦六种消息*：`initialize` 用缓存的响应按浏览器 id 回放；`initialized` 与 `shutdown` / `exit` 吞掉；`didOpen` / `didChange` 先把全文写进容器再转发。其余请求、通知、响应原样双向转发。

*关闭信号是一条合成的 `exit` 通知*（`params.reason` 带原因）：RA 管道断开时会话不可再用，服务端往 `lsp` 信封里塞一条 `exit`，前端收到即清高亮、诊断并 reject 挂起的请求。不用 `shutdown` —— 它是带 `id` 的请求，会诱发一次无意义的应答。

*池只做状态机*：空闲数不超过下限（默认 1）、取用时用会话自己的探针查活、坏的就销毁重造、空闲超 TTL 由回收循环 `docker rm -f`。

*语义 token 与诊断都是前端主动拉的*：`initialize` 响应里的 legend 由前端用来把索引翻成名字；诊断走 `textDocument/diagnostic`，`unchanged` 保留上一份，同时兼容推送式 `publishDiagnostics`。服务端不解析这两类结果。
```
````

- [ ] **Step 3: 更新前端一节的重连说明**

把第 201 行（`*重连不擦高亮*…`）整段替换为：

```typst
*重连重新握手*：页面空闲断开后重连会重新 `hello` → `lsp_open`，前端销毁旧 `LspClient`、清空挂起请求，然后对新会话重新 `initialize` / `didOpen`。已有 decoration 可以保留到新结果到达 —— 新会话的 token 一定是针对当前文档的，不再需要「`lsp_ready` 之前丢弃 token」的规则。
```

- [ ] **Step 4: 全量 Rust 检查**

```shell
cargo test -p rust2genshin-playground
```

Expected: 全部 PASS。

再查残留引用：

```shell
grep -rn "LspTokens\|lsp_tokens\|LspChange\|lsp_change\|lsp_completion\|LspDiagnostics\|lsp_diagnostics\|request_completion\|did_change(" playground docs/typst/play
```

Expected: 无输出（`didChange` 字符串本身仍在，匹配的是旧方法名 `did_change(`）。

- [ ] **Step 5: 起文档站**

后台运行：

```shell
cargo run -p rust2genshin-docs --bin watch
```

打开 `http://127.0.0.1:3000/rust2genshin/play/`（按构建输出的实际路径为准），确认页面无 404、`lsp-client.js` 与 `script.js` 加载成功。**不要**另起 `python -m http.server`，也不要跑 `--bin compile`。

- [ ] **Step 6: 浏览器手工验收**

用 Playwright 或本机浏览器逐项确认：

1. 打开摆弄页，状态从「分析中…」变成「已就绪 · 分析在线」，代码出现语义高亮；
2. 编辑器里改出一个类型错误，波浪线在约 300 ms 后出现；改回去后消失；
3. 在 `sc::` 后按 `Alt-/`，补全弹窗出现，选中后插入的名字与之前一致；
4. 空闲 90 秒触发断连，再打字 → 自动重连，高亮恢复；
5. 在控制台临时执行 `client.send({type:"lsp", message:{jsonrpc:"2.0", id:999, method:"textDocument/definition", params:{textDocument:{uri:"file:///opt/ra/ws/demo/src/lib.rs"},position:{line:0,character:4}}}})`，Network/WebSocket 帧里能看到 `id:999` 的原样响应（证明传输通了，不要求 UI 有反应）。

若本机有 docker 与烘好的镜像，再跑一次真会话集成测试：

```shell
cargo test -p rust2genshin-playground lsp_session_forwards_raw_messages_and_is_reused_after_disconnect -- --ignored
```

Expected: PASS。

- [ ] **Step 7: Commit**

```shell
git add docs/typst/arch/play.typ
git commit -m "docs(play): 更新 LSP 原样转发的协议与会话说明

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Self-Review

- **Spec coverage:** 协议信封与版本（Task 1）、拦截清单与 id 空间（Task 4）、会话边界与就绪判据（Task 3）、前端传输/legend/诊断/补全/重连（Task 5）、script 接线（Task 6）、删除清单（Task 4/5/6）、文档（Task 7）、测试 1–10（Task 2/3/4，其中 10 由 ignored 集成测试覆盖）。
- **Placeholder scan:** 没有 TBD/TODO；所有测试与实现都给出了完整代码。
- **Type consistency:** `LspRoute` 四个变体在路由函数、ws 分支、测试里一致；`send_raw` / `request` / `did_close` / `initialize_result` 的签名在 Task 2 定义、Task 3/4 使用；前端 `label_details` 与 `script.js` 现有读取一致。
- **Review Focus:** 五条都在对应任务的测试里有落点；第 5 条依赖真 docker，落在 ignored 集成测试与手工重连检查。
