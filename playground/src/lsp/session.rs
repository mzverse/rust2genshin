//! 一个 RA 会话:拆帧、配对服务端自己发出的请求、其余原样转发。
//!
//! 见 `docs/superpowers/specs/2026-10-19-play-lsp-raw-proxy-design.md`。
//!
//! 分工:
//!
//! - `exec::RaProcess` 只会「发一帧 / 收一帧」;
//! - 这里起一个后台 reader,把收到的帧按 JSON-RPC 的 `method` / `id` 分流:
//!   带 `method` 的一律转发(带 `id` 且**没有订阅者**时额外替它回一条
//!   `result: null` —— 待命阶段没有浏览器,RA 收不到应答会一直等);
//!   命中 [`Pending`] 表的响应交给等它的调用方;剩下的(浏览器自己发的
//!   请求的响应)原样转发。
//!
//! **这里不做任何翻译**:补全、诊断、语义 token 都在浏览器里按 JSON-RPC
//! 解析,前端拿到的就是 RA 发出的那一帧。
//!
//! 两种请求共用一条 JSON-RPC id 空间:内部请求(`initialize`、就绪探测、
//! `ping`)从 [`INTERNAL_ID_BASE`] 起单调递增,浏览器从 1 开始计数,不会
//! 撞进高半区 —— 因此服务端不需要维护「客户端 id → 内部 id」的反向映射。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{broadcast, oneshot};

use crate::docker::RA_WORKDIR;
use crate::lsp::exec::RaProcess;

/// reader 的单次 `recv` 上限。它既是「多久回来看一眼 alive」,也是
/// 「沉默」与「死了」的分界:`Ok(None)` 继续,`Err(ChildExited)` 才收工。
const RECV_TIMEOUT: Duration = Duration::from_millis(50);

/// `initialize` 的响应上限。RA 要拉 crate graph,真实冷启动 6–11s;
/// 30s 是给慢机器的余量,超了就换会话。
const INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// 我们在 `initialize` 里上报的 token 类型。
///
/// RA **不按它编码** —— 它回自己那份 legend,并按自己那份的下标发 token。
/// 前端拿到缓存的 `initialize` 结果后按**名字**翻译,见 `lsp-client.js`。
/// 这张表只是「我们认得这些名字」的声明。
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

/// 内部请求 id 的起点。浏览器 id 从 1 起,正常不会到达这里。
pub const INTERNAL_ID_BASE: u64 = 0x4000_0000;

/// `demo/src/lib.rs` 在 RA workspace 里的 URI。必须和 `docker::RA_WORKDIR`
/// 拼出来的一致 —— 有测试钉住。
pub const DOC_URI: &str = "file:///opt/ra/ws/demo/src/lib.rs";

/// 会话推给上层的事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// RA 发来的一帧,原样转发给浏览器。
    Message(Value),
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
    #[error("RA 响应不合约:{0}")]
    BadResponse(String),
}

/// 已经发出去、正在等响应的请求。由 JSON-RPC id 索引 —— 只在待命阶段
/// (`initialize`、就绪探测、`ping`)非空,交棒给浏览器时必为空。
enum Pending {
    Response(oneshot::Sender<Value>),
}

pub struct Session {
    proc: Arc<RaProcess>,
    /// 事件广播端。池里的会话要跨多条 WebSocket 连接复用,单消费者的
    /// `mpsc` 做不到 —— 第一条连接断开时接收端被 drop,第二条就再也
    /// 收不到任何东西。`broadcast` 允许任意多个 `subscribe()`;没有
    /// 接收者时 `send` 返回 `Err`,那是正常的(会话在池里待命)。
    events_tx: broadcast::Sender<Event>,
    pending: Arc<Mutex<HashMap<u64, Pending>>>,
    /// reader 看到 `ChildExited` 时置 false。`is_alive` 只看它 ——
    /// 每次询问都去 ping 一下会把 RA 打慢。
    alive: Arc<AtomicBool>,
    /// 内部请求 id,从 [`INTERNAL_ID_BASE`] 单调递增。
    next_internal_id: Arc<AtomicU64>,
    /// 待命阶段 `initialize` 的 `result`,交棒后按浏览器的 id 回放。
    initialize_result: Arc<Mutex<Option<Value>>>,
    /// 当前文档在 RA 侧是否处于打开状态。交棒 / 归还前据此补 `didClose`。
    doc_open: AtomicBool,
    /// 后台读任务。`Drop` 时 abort —— 否则它会攥着 `Arc<RaProcess>`
    /// 和事件 channel 活到进程结束。
    reader: tokio::task::JoinHandle<()>,
}

impl Session {
    /// 包住一个已经起好的 RA 进程,并启动后台 reader。
    pub fn new(proc: Arc<RaProcess>) -> Self {
        let (tx, _rx) = broadcast::channel(64);
        let alive = Arc::new(AtomicBool::new(true));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let next_internal_id = Arc::new(AtomicU64::new(INTERNAL_ID_BASE));
        let reader = tokio::spawn(reader_loop(
            Arc::clone(&proc),
            tx.clone(),
            Arc::clone(&alive),
            Arc::clone(&pending),
        ));
        Self {
            proc,
            events_tx: tx,
            initialize_result: Arc::new(Mutex::new(None)),
            doc_open: AtomicBool::new(false),
            pending,
            alive,
            next_internal_id,
            reader,
        }
    }

    /// 发 `initialize` 并**等响应**,成功后再发 `initialized` 通知,
    /// 并把 `result` 缓存下来。
    ///
    /// capabilities 里必须声明 `semanticTokens`(不然 RA 不给
    /// `semanticTokensProvider`,前端高亮一片空白),且要声明支持
    /// **拉取式**诊断 —— RA 1.10x 不再推 `publishDiagnostics`,不声明也不
    /// 拉的话,波浪线永远停在第一次那份内容上。`snippetSupport: false`
    /// —— 前端不展开占位符。
    pub async fn initialize(&self) -> Result<(), LspError> {
        let params = json!({
            "processId": Value::Null,
            "rootUri": format!("file://{RA_WORKDIR}"),
            "capabilities": {
                "textDocument": {
                    "synchronization": { "didSave": false },
                    "completion": {
                        "completionItem": {
                            // 前端不展开占位符。
                            "snippetSupport": false,
                            // 让 RA 把 `(as Trait)` / `(alias a, b)` 放进
                            // `labelDetails` 而不是拼进 `label` —— 拼进
                            // `label` 的话选中后会被**插进代码**,而且
                            // `label` 也不再是合法的标识符前缀。
                            "labelDetailsSupport": true
                        }
                    },
                    // 声明支持**拉取式**诊断。
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
        // 缓存整份 `result`:交棒后浏览器的 `initialize` 不转发到 RA,
        // 用这份按浏览器的 id 回放。前端也靠它里面的 legend 把 token
        // 下标翻译成名字。
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

    /// 打开文档(全文)。待命阶段灌占位内容用;交棒后浏览器自己的
    /// `didOpen` 由 ws 原样转发。这里不主动拉 token / 诊断 ——
    /// 浏览器会自己发那两个请求。
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

    /// 关闭当前文档。幂等:没有打开的文档直接返回。
    ///
    /// 交棒(`LspService::acquire`)与归还(`ws` 的收尾)都要调:
    /// 池里的会话跨连接复用,只有把上一份 overlay 关掉,新浏览器的
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

    /// 原样转发一条 LSP 报文,并维护文档开关状态。
    pub async fn send_raw(&self, msg: &Value) -> Result<(), LspError> {
        match msg.get("method").and_then(Value::as_str) {
            Some("textDocument/didOpen") => self.doc_open.store(true, Ordering::SeqCst),
            Some("textDocument/didClose") => self.doc_open.store(false, Ordering::SeqCst),
            _ => {}
        }
        self.send_json(msg).await
    }

    /// 订阅事件。每次调用都拿到一个独立的接收端 —— 会话在池里跨连接
    /// 复用时,前后两条连接各自订阅,互不影响。
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events_tx.subscribe()
    }

    /// 发一个服务端自己的请求并等响应(完整响应,`error` 也算有响应)。
    /// 只在待命阶段使用:`initialize`、就绪探测、`ping`。
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
            // oneshot 的发送端被 drop = reader 退出了 = RA 死了。
            Ok(Err(_)) => Err(LspError::Closed),
            Err(_) => {
                self.unregister(id);
                Err(LspError::Timeout)
            }
        }
    }

    /// 存活性检查。**必须走 pending 表**:直接读 `RaProcess` 会和后台
    /// reader 抢同一根管道,把帧偷走,或让健康会话被误判死亡。
    /// RA 对未知方法回 `error` 响应 —— 有响应就是活着。
    pub async fn ping(&self, timeout: Duration) -> Result<(), LspError> {
        self.request("r2g/ping", Value::Null, timeout).await.map(|_| ())
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

/// 后台读循环:把 RA 的每一帧分到 pending 表或转发端。
async fn reader_loop(
    proc: Arc<RaProcess>,
    tx: broadcast::Sender<Event>,
    alive: Arc<AtomicBool>,
    pending: Arc<Mutex<HashMap<u64, Pending>>>,
) {
    while alive.load(Ordering::SeqCst) {
        match proc.recv(RECV_TIMEOUT).await {
            Ok(Some(raw)) => {
                // 不是 JSON 的帧:忽略。RA 的 stdout 上只该有 LSP 帧。
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    dispatch(v, &proc, &tx, &pending).await;
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
    // 清空 pending:正在 `await` 的调用方(比如 `initialize`)会立刻
    // 拿到 oneshot 断开(= `Closed`),不必等满 `INIT_TIMEOUT`。
    // 不清的话 `Session` 还持着这颗 `Arc`,sender 一直不 drop。
    pending
        .lock()
        .expect("Mutex 不该中毒:持锁段不 panic")
        .clear();
    // `tx` 在这里 drop —— 事件广播端关闭,订阅者据此知道会话没了。
}

/// 把 RA 的一帧分到 pending 表或转发端。
///
/// - 带 `method` 的是通知或 RA 发来的请求。请求(带非 null `id`)在
///   **没有订阅者**时由服务端替它回 `null`(待命 / 交棒间隙)—— 静默丢弃
///   会让 RA 以为客户端挂了;有订阅者时原样转给前端、由前端回答,服务端
///   不抢答(抢答会和浏览器的响应撞成两条同 id 的响应)。
/// - 没有 `method` 的是响应:命中 [`Pending`] 的是服务端自己的请求,
///   交给 oneshot;没命中的是浏览器请求的响应,原样转发。
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
        // 既无 method 也无数字 id 的怪帧:转发出去,前端会忽略。
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

        /// drop 掉 stdout 的 sender,模拟 RA 死掉 / 管道关闭。
        fn disconnect_stdout(&self) {
            *self
                .stdout_tx
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic") = None;
        }

        /// 等某条消息真的写出去,返回那条请求的 `id`(没有 id 的返回 `None`)。
        /// 主要给「后台任务要先看到请求才回帧」用,避免回帧早于 pending
        /// 登记被 reader 丢掉。
        ///
        /// 返回 id 是为了回响应时不必猜 id —— 内部 id 由会话分配,测试
        /// 写死 `INTERNAL_ID_BASE` 只在「第一次请求」时成立。
        async fn wait_until_written(&self, needle: &str) -> Option<u64> {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            loop {
                let bytes = self
                    .written
                    .lock()
                    .expect("Mutex 不该中毒:持锁段不 panic")
                    .clone();
                if String::from_utf8_lossy(&bytes).contains(needle) {
                    // 从后往前找:同一 suffix 的方法可能被写过多次(比如
                    // didChange 触发的重复 token 请求),要的是最后一条。
                    let mut dec = frame::FrameDecoder::new();
                    let frames = dec.push(&bytes).expect("测试写出的都是完整帧");
                    return frames.iter().rev().find_map(|b| {
                        let v: serde_json::Value = serde_json::from_slice(b).ok()?;
                        if !v.to_string().contains(needle) {
                            return None;
                        }
                        v.get("id").and_then(serde_json::Value::as_u64)
                    });
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
    async fn next_event(rx: &mut broadcast::Receiver<Event>) -> Event {
        match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
            // 测试里消费得足够快,落后说明 channel 容量或测试本身有问题。
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                panic!("测试里不该落后,丢了 {n} 条事件")
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => panic!("事件 channel 不该被关"),
            Ok(Ok(e)) => e,
            Err(_) => panic!("1s 内该收到事件"),
        }
    }

    #[tokio::test]
    async fn reader_exit_wakes_pending_initialize() {
        let (fake, session) = setup().await;
        let disconnect = tokio::spawn({
            let fake = Arc::clone(&fake);
            async move {
                let _ = fake.wait_until_written("\"method\":\"initialize\"").await;
                fake.disconnect_stdout();
            }
        });

        let start = tokio::time::Instant::now();
        let res = tokio::time::timeout(Duration::from_secs(5), session.initialize())
            .await
            .expect("RA 死了要立刻醒,不能等满 INIT_TIMEOUT");
        let elapsed = start.elapsed();
        disconnect.await.expect("disconnect 任务不该 panic");
        assert!(
            matches!(res, Err(LspError::Closed)),
            "期望 Closed,实际:{res:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "耗时 {elapsed:?},不该接近 INIT_TIMEOUT"
        );
    }

    #[tokio::test]
    async fn session_ping_goes_through_the_pending_map() {
        let (fake, session) = setup().await;
        let feeder = tokio::spawn({
            let fake = Arc::clone(&fake);
            async move {
                // 等 ping 请求真的写出去,再按它的 id 回一条 **error** 响应:
                // 未知方法 RA 就是这么回的 —— 有响应即证明活着。
                let id = fake
                    .wait_until_written("\"method\":\"r2g/ping\"")
                    .await
                    .expect("ping 请求该带 id");
                let resp = serde_json::json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32601, "message": "method not found" }
                });
                fake.sender()
                    .send(frame::encode(&resp.to_string()))
                    .expect("UnboundedSender 不该 send 失败");
            }
        });
        session
            .ping(Duration::from_secs(1))
            .await
            .expect("error 响应也算活着");
        feeder.await.expect("feeder 不该 panic");

        // 沉默必须超时,不能 hang。
        let (fake2, session2) = setup().await;
        assert!(session2.ping(Duration::from_millis(50)).await.is_err());
        drop(fake2);
    }

    /// 交棒后浏览器的 `initialize` 不转发到 RA,服务端拿这份缓存按浏览器的
    /// id 回放 —— 缓存漏了就是「分析器还没初始化完」。
    #[tokio::test]
    async fn initialize_caches_the_result_for_replay() {
        let (fake, session) = setup().await;
        let feeder = tokio::spawn({
            let fake = Arc::clone(&fake);
            async move {
                let id = fake
                    .wait_until_written("\"method\":\"initialize\"")
                    .await
                    .expect("initialize 请求该带 id");
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "semanticTokensProvider": {
                                "legend": { "tokenTypes": ["keyword", "function"] }
                            }
                        }
                    }
                });
                fake.sender()
                    .send(frame::encode(&resp.to_string()))
                    .expect("UnboundedSender 不该 send 失败");
            }
        });
        session.initialize().await.expect("initialize 该成功");
        feeder.await.expect("feeder 不该 panic");

        let cached = session
            .initialize_result()
            .expect("initialize 之后必须有缓存");
        assert_eq!(
            cached["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"],
            serde_json::json!(["keyword", "function"]),
            "缓存的必须是 RA 报的 legend 原文,前端按名字翻译靠它"
        );
        // initialized 也该发出去。
        assert!(
            fake.written_bodies()
                .iter()
                .any(|b| b["method"] == "textDocument/diagnostic"
                    || b["method"] == "initialized"),
            "initialize 之后要补一条 initialized 通知"
        );
    }

    #[tokio::test]
    async fn did_open_and_close_track_the_document() {
        let (fake, session) = setup().await;
        session.did_close().await.expect("没有文档时 didClose 应为空操作");
        assert!(
            !fake.written_bodies().iter().any(|b| b["method"] == "textDocument/didClose"),
            "没有打开的文档不该发 didClose"
        );

        session.did_open("fn a() {}").await.expect("didOpen 该成功");
        session.did_close().await.expect("didClose 该成功");
        // 幂等:再关一次不该再发。
        session.did_close().await.expect("第二次 didClose 该成功");

        let bodies = fake.written_bodies();
        assert_eq!(
            bodies
                .iter()
                .filter(|b| b["method"] == "textDocument/didClose")
                .count(),
            1,
            "didClose 必须幂等,否则 RA 会对没打开的文档报错"
        );
    }

    #[tokio::test]
    async fn forwarded_messages_reach_ra_verbatim() {
        let (fake, session) = setup().await;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "textDocument/completion",
            "params": { "position": { "line": 3, "character": 12 } }
        });
        session.send_raw(&msg).await.expect("转发该成功");
        let bodies = fake.written_bodies();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0], msg, "转发必须原样,服务端不重写任何字段");
    }

    #[tokio::test]
    async fn ra_messages_are_forwarded_verbatim() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "result": { "items": [{ "label": "hello" }] }
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
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        let msg = serde_json::json!({ "jsonrpc": "2.0", "id": 424242, "result": null });
        fake.sender()
            .send(frame::encode(&msg.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        match next_event(&mut events).await {
            Event::Message(v) => assert_eq!(v, msg),
        }
    }

    /// 待命阶段没人订阅:RA 的请求必须由服务端替它回 null,否则 RA
    /// 一直等,会话永远不就绪。
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
        fake.wait_until_written("\"id\":99")
            .await
            .expect("服务端该替 RA 的请求回一条 null");
        let bodies = fake.written_bodies();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0]["result"], Value::Null);

        // 有前端时原样转发,服务端不抢答 —— 抢答会和浏览器的响应
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
}
