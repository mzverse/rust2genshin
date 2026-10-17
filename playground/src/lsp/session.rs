//! 一个 RA 会话:在上面套 LSP 的请求/通知语义,并把 RA 主动推的消息
//! (诊断、语义 token)转成事件。
//!
//! 见 `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md`。
//!
//! 分工:
//!
//! - `exec::RaProcess` 只会「发一帧 / 收一帧」;
//! - 这里起一个后台 reader 任务,把收到的帧按 JSON-RPC 的 id / method
//!   分流:带 id 且命中 pending 表的是响应,`publishDiagnostics` 是诊断,
//!   其余(日志、progress)忽略。
//!
//! 两种请求共用一条 JSON-RPC id 空间:内部请求(`initialize`、
//! `semanticTokens/full`)从 [`INTERNAL_ID_BASE`] 起单调递增,补全请求
//! 直接用前端给的 id。前端从 1 开始计数,不会撞进高半区 —— 因此服务端
//! 不需要维护「客户端 id → 内部 id」的反向映射。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::docker::RA_WORKDIR;
use crate::lsp::exec::RaProcess;

/// reader 的单次 `recv` 上限。它既是「多久回来看一眼 alive」,也是
/// 「沉默」与「死了」的分界:`Ok(None)` 继续,`Err(ChildExited)` 才收工。
const RECV_TIMEOUT: Duration = Duration::from_millis(50);

/// `initialize` 的响应上限。RA 要拉 crate graph,真实冷启动 6–11s;
/// 30s 是给慢机器的余量,超了就换会话。
const INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// LSP 标准 token 类型的**固定顺序**。前端(Task 10)用同一份顺序把索引
/// 映射成样式 class —— 两边顺序不一致,高亮就会整体错位。
const TOKEN_TYPES: [&str; 23] = [
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "macro",
    "keyword",
    "modifier",
    "comment",
    "string",
    "number",
    "regexp",
    "operator",
    "decorator",
];

/// 内部请求 id 的起点。前端 id 从 1 起,正常不会到达这里;真到了就拒绝。
pub const INTERNAL_ID_BASE: u64 = 0x4000_0000;

/// `demo/src/lib.rs` 在 RA workspace 里的 URI。必须和 `docker::RA_WORKDIR`
/// 拼出来的一致 —— 有测试钉住。
pub const DOC_URI: &str = "file:///opt/ra/ws/demo/src/lib.rs";

/// 一条补全候选。只取前端渲染要用的三个字段;`kind` 是 LSP 的数字枚举,
/// 原样传,映射成样式是前端的活。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
}

/// 会话推给上层的事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 一条 `publishDiagnostics` 里的诊断。位置是 LSP 的 0 基行列。
    Diagnostic {
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
        severity: u8,
        message: String,
    },
    /// `semanticTokens/full` 的 delta 数组,**原样透传**,不拆。
    Tokens(Vec<u32>),
    /// 补全响应。`request_id` 是前端自己的 id。
    Completed {
        request_id: u32,
        items: Vec<CompletionItem>,
    },
}

#[derive(Debug, Error)]
pub enum LspError {
    #[error("写 RA 的 stdin 失败:{0}")]
    Exec(String),
    #[error("等 RA 响应超时")]
    Timeout,
    /// reader 那边看到子进程/管道断了:会话已死,换一个。
    #[error("会话已死(RA 退出)")]
    Closed,
    /// 补全请求 id 落进了内部保留区间。
    #[error("补全请求 id {0} 不能 >= {INTERNAL_ID_BASE}")]
    BadRequestId(u32),
    #[error("RA 响应不合约:{0}")]
    BadResponse(String),
}

/// 已经发出去、正在等响应的请求。由 JSON-RPC id 索引:
///
/// - `Response`:调用方 `await` 一个 oneshot(`initialize`);
/// - `Tokens` / `Completion`:响应没有人 await,reader 直接转成 `Event`。
enum Pending {
    Response(oneshot::Sender<Value>),
    Tokens,
    /// `request_id` 是前端给的 id —— 回显在 `Event::Completed` 里。
    Completion {
        request_id: u32,
    },
}

pub struct Session {
    proc: Arc<RaProcess>,
    /// 事件接收端只取一次。`Mutex<Option<..>>` 是为了 `events(&self)`
    /// 能把它「拿走」交给单个消费者。
    events_rx: Mutex<Option<mpsc::UnboundedReceiver<Event>>>,
    pending: Arc<Mutex<HashMap<u64, Pending>>>,
    /// reader 看到 `ChildExited` 时置 false。`is_alive` 只看它 ——
    /// 每次询问都去 ping 一下会把 RA 打慢。
    alive: Arc<AtomicBool>,
    /// 内部请求 id,从 [`INTERNAL_ID_BASE`] 单调递增。
    next_internal_id: AtomicU64,
    /// didOpen / didChange 共用的文档版本号。
    version: AtomicU64,
    /// 后台读任务。`Drop` 时 abort —— 否则它会攥着 `Arc<RaProcess>`
    /// 和事件 channel 活到进程结束。
    reader: tokio::task::JoinHandle<()>,
}

impl Session {
    /// 包住一个已经起好的 RA 进程,并启动后台 reader。
    pub fn new(proc: Arc<RaProcess>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let alive = Arc::new(AtomicBool::new(true));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let reader = tokio::spawn(reader_loop(
            Arc::clone(&proc),
            tx,
            Arc::clone(&alive),
            Arc::clone(&pending),
        ));
        Self {
            proc,
            events_rx: Mutex::new(Some(rx)),
            pending,
            alive,
            next_internal_id: AtomicU64::new(INTERNAL_ID_BASE),
            version: AtomicU64::new(0),
            reader,
        }
    }

    /// 发 `initialize` 并**等响应**,成功后再发 `initialized` 通知。
    ///
    /// capabilities 里必须声明 `semanticTokens`(不然 RA 不给
    /// `semanticTokensProvider`,前端高亮一片空白),且 `tokenTypes`
    /// 顺序与前端一致。`snippetSupport: false` —— 前端不展开占位符。
    pub async fn initialize(&self) -> Result<(), LspError> {
        let id = self.next_id();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .insert(id, Pending::Response(resp_tx));

        let params = json!({
            "processId": Value::Null,
            "rootUri": format!("file://{RA_WORKDIR}"),
            "capabilities": {
                "textDocument": {
                    "synchronization": { "didSave": false },
                    "completion": {
                        "completionItem": { "snippetSupport": false }
                    },
                    "semanticTokens": {
                        "requests": { "full": true },
                        "formats": ["relative"],
                        "tokenTypes": TOKEN_TYPES,
                        "tokenModifiers": [],
                    }
                }
            }
        });
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": params,
        });
        if let Err(e) = self.send_json(&msg).await {
            self.unregister(id);
            return Err(e);
        }

        let resp = match tokio::time::timeout(INIT_TIMEOUT, resp_rx).await {
            Ok(Ok(v)) => v,
            // oneshot 的发送端被 drop = reader 退出了 = RA 死了。
            Ok(Err(_)) => return Err(LspError::Closed),
            Err(_) => {
                self.unregister(id);
                return Err(LspError::Timeout);
            }
        };
        if let Some(err) = resp.get("error") {
            return Err(LspError::BadResponse(err.to_string()));
        }

        self.send_json(&json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {},
        }))
        .await
    }

    /// 打开文档(全文),版本从 1 起。
    pub async fn did_open(&self, text: &str) -> Result<(), LspError> {
        self.version.store(1, Ordering::SeqCst);
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

    /// 全文替换(不带 `range`),然后主动要一次 `semanticTokens/full`。
    ///
    /// RA 不会自己推 token —— 不主动要,前端就一直没有高亮。
    pub async fn did_change(&self, text: &str) -> Result<(), LspError> {
        let version = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        self.send_json(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": DOC_URI, "version": version },
                // 全文替换:只有一条 change、没有 range。
                "contentChanges": [{ "text": text }],
            }
        }))
        .await?;

        let id = self.next_id();
        self.pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .insert(id, Pending::Tokens);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/semanticTokens/full",
            "params": { "textDocument": { "uri": DOC_URI } },
        });
        if let Err(e) = self.send_json(&req).await {
            self.unregister(id);
            return Err(e);
        }
        Ok(())
    }

    /// 发一个补全请求。**id 用前端给的 `request_id` 原样当 JSON-RPC id**,
    /// 响应由 reader 配对成 [`Event::Completed`]。
    ///
    /// 前端 id 从 1 起,不会落进内部保留区间;真发来了就报错,不静默接受。
    pub async fn request_completion(
        &self,
        request_id: u32,
        line: u32,
        column: u32,
    ) -> Result<(), LspError> {
        if u64::from(request_id) >= INTERNAL_ID_BASE {
            return Err(LspError::BadRequestId(request_id));
        }
        let id = u64::from(request_id);
        self.pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .insert(id, Pending::Completion { request_id });
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": DOC_URI },
                "position": { "line": line, "character": column },
            }
        });
        if let Err(e) = self.send_json(&req).await {
            self.unregister(id);
            return Err(e);
        }
        Ok(())
    }

    /// 事件接收端。**只能调一次** —— channel 只有一个接收端。
    pub fn events(&self) -> mpsc::UnboundedReceiver<Event> {
        self.events_rx
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .take()
            .expect("Session::events() 只能调用一次")
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// 主动关:杀子进程,并让 reader 在下一轮 `recv` 后收工。
    pub async fn shutdown(&self) {
        self.proc.shutdown().await;
        self.alive.store(false, Ordering::SeqCst);
    }

    fn next_id(&self) -> u64 {
        self.next_internal_id.fetch_add(1, Ordering::SeqCst)
    }

    fn unregister(&self, id: u64) {
        self.pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .remove(&id);
    }

    async fn send_json(&self, msg: &Value) -> Result<(), LspError> {
        self.proc
            .send(&msg.to_string())
            .await
            .map_err(|e| LspError::Exec(e.to_string()))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // reader 被 abort 时它的 `tx` 随之 drop,事件 channel 关闭。
        self.reader.abort();
    }
}

/// 后台读循环:把 RA 的每一帧分到 pending 表或事件里。
async fn reader_loop(
    proc: Arc<RaProcess>,
    tx: mpsc::UnboundedSender<Event>,
    alive: Arc<AtomicBool>,
    pending: Arc<Mutex<HashMap<u64, Pending>>>,
) {
    while alive.load(Ordering::SeqCst) {
        match proc.recv(RECV_TIMEOUT).await {
            Ok(Some(raw)) => {
                // 不是 JSON 的帧:忽略。RA 的 stdout 上只该有 LSP 帧。
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    dispatch(v, &tx, &pending);
                }
            }
            // 沉默(RA 还活着只是没回):继续等。
            Ok(None) => continue,
            // `ChildExited` 是 RA 死了;其它 `ExecError`(拆帧失败)也
            // 说明这条流不可信 —— 两种情况都按「会话已死」处理。
            Err(e) => {
                tracing::debug!("RA 会话 reader 退出:{e}");
                alive.store(false, Ordering::SeqCst);
                break;
            }
        }
    }
    // `tx` 在这里 drop —— 事件 channel 关闭,上层据此知道会话没了。
}

fn dispatch(v: Value, tx: &mpsc::UnboundedSender<Event>, pending: &Mutex<HashMap<u64, Pending>>) {
    // 带 method 的是通知或服务端发来的请求。诊断是唯一要转发的;
    // window/logMessage、$/progress 以及服务端请求(注册能力、刷新
    // token 等)一律忽略。
    if let Some(method) = v.get("method").and_then(Value::as_str) {
        if method == "textDocument/publishDiagnostics" {
            emit_diagnostics(&v, tx);
        }
        return;
    }

    let Some(id) = v.get("id").and_then(Value::as_u64) else {
        return;
    };
    let pending = pending
        .lock()
        .expect("Mutex 不该中毒:持锁段不 panic")
        .remove(&id);
    let Some(pending) = pending else {
        return;
    };
    match pending {
        Pending::Response(resp_tx) => {
            let _ = resp_tx.send(v);
        }
        Pending::Tokens => {
            // `delta` 数组原样透传:拆成 (start, len, type, modifiers)
            // 是前端 CodeMirror 扩展的活。
            let data = v
                .get("result")
                .and_then(|r| r.get("data"))
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<u32>>()
                })
                .unwrap_or_default();
            let _ = tx.send(Event::Tokens(data));
        }
        Pending::Completion { request_id } => {
            let items = parse_completion_items(v.get("result"));
            let _ = tx.send(Event::Completed { request_id, items });
        }
    }
}

fn emit_diagnostics(v: &Value, tx: &mpsc::UnboundedSender<Event>) {
    let Some(list) = v
        .get("params")
        .and_then(|p| p.get("diagnostics"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for d in list {
        if let Some(event) = diagnostic_from(d) {
            let _ = tx.send(event);
        }
    }
}

fn diagnostic_from(d: &Value) -> Option<Event> {
    let range = d.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some(Event::Diagnostic {
        line: start.get("line")?.as_u64()? as u32,
        column: start.get("character")?.as_u64()? as u32,
        end_line: end.get("line")?.as_u64()? as u32,
        end_column: end.get("character")?.as_u64()? as u32,
        // LSP 省略 severity 时按 Error(1)算。
        severity: d.get("severity").and_then(Value::as_u64).unwrap_or(1) as u8,
        message: d
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

/// completion 的 `result` 有两种形状:`CompletionList { items }` 或
/// 裸数组(老客户端/某些服务端)。两种都收。
fn parse_completion_items(result: Option<&Value>) -> Vec<CompletionItem> {
    let Some(result) = result else {
        return Vec::new();
    };
    let items: Vec<&Value> = if let Some(arr) = result.as_array() {
        arr.iter().collect()
    } else {
        result
            .get("items")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    };
    items
        .into_iter()
        .filter_map(|it| {
            Some(CompletionItem {
                label: it.get("label")?.as_str()?.to_string(),
                kind: it.get("kind").and_then(Value::as_u64).map(|k| k as u32),
                detail: it.get("detail").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use crate::lsp::exec::{ExecError, RaCommand};
    use crate::lsp::frame;

    /// 假 docker:把写出的字节攒起来,stdout 端交给测试塞预设帧。
    #[derive(Default)]
    struct FakeDocker {
        written: StdMutex<Vec<u8>>,
        stdout_tx: StdMutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    }

    #[async_trait]
    impl RaCommand for FakeDocker {
        async fn spawn(&self, _args: &[String]) -> Result<(), ExecError> {
            Ok(())
        }
        async fn write(&self, bytes: &[u8]) -> Result<(), ExecError> {
            self.written
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic")
                .extend_from_slice(bytes);
            Ok(())
        }
        async fn kill(&self) {}
        fn take_stdout(&self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
            let (tx, rx) = mpsc::unbounded_channel();
            *self
                .stdout_tx
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic") = Some(tx);
            Some(rx)
        }
    }

    impl FakeDocker {
        /// 往 RA 的 stdout 塞字节(**必须在 `RaProcess::start` 之后调**)。
        fn sender(&self) -> mpsc::UnboundedSender<Vec<u8>> {
            self.stdout_tx
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic")
                .as_ref()
                .expect("start() 之前没有 stdout sender")
                .clone()
        }

        /// 已写出的消息,拆帧后按 JSON 解析。写入与断言之间没有 reader
        /// 参与(`send` 里 `write` 是 await 完成的),所以这里不用等。
        fn written_bodies(&self) -> Vec<serde_json::Value> {
            let bytes = self
                .written
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic")
                .clone();
            let mut dec = frame::FrameDecoder::new();
            let frames = dec.push(&bytes).expect("测试写出的都是完整帧");
            frames
                .iter()
                .map(|b| serde_json::from_slice(b).expect("写出的正文该是 JSON"))
                .collect()
        }

        /// 等某条消息真的写出去。主要给「后台任务要先看到请求才回帧」用,
        /// 避免回帧早于 pending 登记被 reader 丢掉。
        async fn wait_until_written(&self, needle: &str) {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            loop {
                let hit = String::from_utf8_lossy(
                    &self.written.lock().expect("Mutex 不该中毒:持锁段不 panic"),
                )
                .contains(needle);
                if hit {
                    return;
                }
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "2s 内等不到写入 {needle}"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    /// 一个接好假 docker 的会话 + 那个 fake(测试要往里塞帧 / 读写入)。
    async fn setup() -> (Arc<FakeDocker>, Session) {
        let fake = Arc::new(FakeDocker::default());
        let cmd: Arc<dyn RaCommand> = Arc::clone(&fake) as Arc<dyn RaCommand>;
        let proc = Arc::new(
            RaProcess::start(cmd, "r2g-ra-test")
                .await
                .expect("FakeDocker::spawn 不该失败"),
        );
        let session = Session::new(proc);
        (fake, session)
    }

    /// 从事件 channel 收一条,1s 收不到就判失败 —— 裸 `recv().await` 挂死
    /// 会变成整个测试超时,看不出是哪一条断言。
    async fn next_event(rx: &mut mpsc::UnboundedReceiver<Event>) -> Event {
        tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("1s 内该收到事件")
            .expect("事件 channel 不该被关")
    }

    #[tokio::test]
    async fn initialize_then_did_open_then_did_change() {
        let (fake, session) = setup().await;
        let tx = fake.sender();

        // initialize 要等响应:后台看到请求写出去之后再回帧。
        let feeder = tokio::spawn({
            let fake = Arc::clone(&fake);
            async move {
                fake.wait_until_written("\"method\":\"initialize\"").await;
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": INTERNAL_ID_BASE,
                    "result": { "capabilities": {} },
                });
                tx.send(frame::encode(&resp.to_string()))
                    .expect("UnboundedSender 不该 send 失败");
            }
        });
        session.initialize().await.expect("initialize 该成功");
        feeder.await.expect("feeder 不该 panic");

        session
            .did_open("fn main() {}")
            .await
            .expect("didOpen 该成功");
        session
            .did_change("fn main() { let x = 1; }")
            .await
            .expect("didChange 该成功");

        let bodies = fake.written_bodies();
        let idx = |m: &str| {
            bodies
                .iter()
                .position(|v| v["method"] == m)
                .unwrap_or_else(|| panic!("没写出 {m}:{bodies:?}"))
        };
        let init = idx("initialize");
        let initialized = idx("initialized");
        let did_open = idx("textDocument/didOpen");
        let did_change = idx("textDocument/didChange");
        assert!(
            init < initialized && initialized < did_open && did_open < did_change,
            "消息顺序不对:{bodies:?}"
        );

        let init = &bodies[init];
        assert_eq!(init["id"].as_u64(), Some(INTERNAL_ID_BASE));
        assert_eq!(init["params"]["processId"], serde_json::Value::Null);
        assert_eq!(
            init["params"]["rootUri"],
            format!("file://{}", crate::docker::RA_WORKDIR),
            "rootUri 必须和容器里的 workdir 一致"
        );
        assert_eq!(
            init["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["snippetSupport"],
            false
        );
        let token_types =
            init["params"]["capabilities"]["textDocument"]["semanticTokens"]["tokenTypes"]
                .as_array()
                .expect("必须声明 semanticTokens.tokenTypes,否则 RA 不给高亮");
        assert_eq!(token_types.len(), 23);
        assert_eq!(token_types[0], "namespace");
        assert_eq!(token_types[22], "decorator");

        let open = &bodies[did_open]["params"]["textDocument"];
        assert_eq!(open["uri"], DOC_URI);
        assert_eq!(
            DOC_URI,
            format!("file://{}/demo/src/lib.rs", crate::docker::RA_WORKDIR)
        );
        assert_eq!(open["languageId"], "rust");
        assert_eq!(open["version"], 1);
        assert_eq!(open["text"], "fn main() {}");

        let change = &bodies[did_change]["params"]["textDocument"];
        assert_eq!(change["version"], 2, "didChange 的 version 要递增");
    }

    #[tokio::test]
    async fn did_change_sends_full_text_not_range() {
        // Review Focus 第 1 条的骨架:didChange 不带 range
        let (fake, session) = setup().await;
        session
            .did_open("fn main() {}")
            .await
            .expect("didOpen 该成功");
        let new_text = "fn main() {\n    let s = \"中文\";\n}";
        session
            .did_change(new_text)
            .await
            .expect("didChange 该成功");

        let bodies = fake.written_bodies();
        let change = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/didChange")
            .expect("该写出 didChange");
        let written = change.to_string();
        assert!(
            !written.contains("\"range\""),
            "didChange 不能带 range:{written}"
        );
        assert!(
            written.contains("\"text\""),
            "didChange 该带 text:{written}"
        );

        let changes = change["params"]["contentChanges"]
            .as_array()
            .expect("contentChanges 该是数组");
        assert_eq!(changes.len(), 1, "全文替换只该有一条 change");
        assert_eq!(changes[0]["text"], new_text);
        assert_eq!(change["params"]["textDocument"]["version"], 2);
    }

    #[tokio::test]
    async fn publish_diagnostics_becomes_an_event() {
        let (fake, session) = setup().await;
        let mut events = session.events();
        let tx = fake.sender();
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": DOC_URI,
                "diagnostics": [{
                    "range": {
                        "start": { "line": 3, "character": 7 },
                        "end": { "line": 3, "character": 12 },
                    },
                    "severity": 2,
                    "message": "unresolved import",
                }],
            },
        });
        tx.send(frame::encode(&notif.to_string()))
            .expect("UnboundedSender 不该 send 失败");

        assert_eq!(
            next_event(&mut events).await,
            Event::Diagnostic {
                line: 3,
                column: 7,
                end_line: 3,
                end_column: 12,
                severity: 2,
                message: "unresolved import".into(),
            }
        );
    }

    #[tokio::test]
    async fn semantic_tokens_pass_through_untouched() {
        // 服务端不拆 delta:Event::Tokens 拿到的就是 RA 给的那串
        let (fake, session) = setup().await;
        let mut events = session.events();
        // didChange 会主动要 semanticTokens/full;这是会话的第一个内部
        // 请求,id = INTERNAL_ID_BASE。
        session
            .did_change("fn main() {}")
            .await
            .expect("didChange 该成功");

        let data: Vec<u32> = vec![0, 0, 3, 4, 0, 1, 1, 5, 1, 0, 2, 3, 1, 22, 0];
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": INTERNAL_ID_BASE,
            "result": { "data": data },
        });
        fake.sender()
            .send(frame::encode(&resp.to_string()))
            .expect("UnboundedSender 不该 send 失败");

        match next_event(&mut events).await {
            Event::Tokens(got) => assert_eq!(got, data, "delta 数组要逐元素原样透传"),
            other => panic!("期望 Event::Tokens,实际:{other:?}"),
        }
    }

    #[tokio::test]
    async fn completion_request_uses_the_callers_id() {
        let (fake, session) = setup().await;
        let mut events = session.events();
        session
            .request_completion(7, 3, 5)
            .await
            .expect("request_completion 该成功");

        let bodies = fake.written_bodies();
        let req = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/completion")
            .expect("该写出 completion 请求");
        assert_eq!(
            req["id"].as_u64(),
            Some(7),
            "补全请求必须用前端给的 id 当 JSON-RPC id"
        );
        assert_eq!(req["params"]["textDocument"]["uri"], DOC_URI);
        assert_eq!(req["params"]["position"]["line"], 3);
        assert_eq!(req["params"]["position"]["character"], 5);

        // 落进内部保留区间要直接报错,不能静默接受。
        assert!(
            session
                .request_completion(INTERNAL_ID_BASE as u32, 0, 0)
                .await
                .is_err(),
            ">= {INTERNAL_ID_BASE} 的 id 该被拒绝"
        );

        // 响应按前端 id 配对,变成 Completed 事件。
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "result": {
                "isIncomplete": false,
                "items": [
                    { "label": "sc", "kind": 9, "detail": "mod sc" },
                    { "label": "advance", "kind": 3 },
                ],
            },
        });
        fake.sender()
            .send(frame::encode(&resp.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        assert_eq!(
            next_event(&mut events).await,
            Event::Completed {
                request_id: 7,
                items: vec![
                    CompletionItem {
                        label: "sc".into(),
                        kind: Some(9),
                        detail: Some("mod sc".into()),
                    },
                    CompletionItem {
                        label: "advance".into(),
                        kind: Some(3),
                        detail: None,
                    },
                ],
            }
        );
    }
}
