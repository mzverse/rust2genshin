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
//!   服务端发来的请求统一回 `result: null`(其中
//!   `workspace/semanticTokens/refresh` 还要重新拉一次 token,
//!   `workspace/diagnostic/refresh` 重新拉一次诊断),其余通知
//!   (日志、progress)忽略。
//!
//! **诊断主要走拉取式**:RA 1.10x 在文档改动后不再推
//! `publishDiagnostics`,只有 `didOpen`/`didChange` 后自己发
//! `textDocument/diagnostic` 才有新结果(见 [`Session::did_open`])。
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
use tokio::sync::{broadcast, oneshot};

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

/// RA 的 semantic token 索引 → 我们 [`TOKEN_TYPES`] 索引的翻译表。
///
/// **为什么必须翻译**:我们在 `initialize` 里上报了自己的 `tokenTypes`,
/// 但 RA **不按它编码** —— 它回自己那份 legend,并且按**自己那份**的下标发
/// token。实测 RA 1.101 回的是字母序:
/// `comment, decorator, enumMember, enum, function, interface, keyword, …`
/// 于是 `keyword` 落在第 6 位,而上面这张表第 6 位是 `typeParameter` ——
/// 不翻译的话 `pub` / `fn` 会被前端当泛型参数染成斜体,`hello_world`
/// 这种函数被染成 `interface` 的颜色。
///
/// 映射按**名字**做,RA 换 legend 顺序不用改我们:名字对上就行。RA 独有的
/// 标点类 token(`brace` / `comma` / `lifetime` …)在我们表里没有对应项,
/// 翻成 [`Self::NO_COLOR`] —— 前端解到越界索引会跳过这一项。
#[derive(Debug, Clone, Default)]
struct TokenLegend {
    /// 下标 = RA 的下标,值 = 我们表里的下标(或 `NO_COLOR`)。
    map: Vec<u32>,
}

impl TokenLegend {
    /// 前端解到不认识的索引就跳过(见 `lsp-client.js` 的 `decodeTokens`)。
    /// 取一个必然越界的值 —— 上面那张表只有 23 项。
    const NO_COLOR: u32 = u32::MAX;

    /// 从 RA 报的 `legend.tokenTypes` 建表。空表 = 直通(行为同改动之前)。
    fn from_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let map = names
            .into_iter()
            .map(|n| {
                TOKEN_TYPES
                    .iter()
                    .position(|t| *t == n.as_ref())
                    .map(|i| i as u32)
                    .unwrap_or(Self::NO_COLOR)
            })
            .collect();
        Self { map }
    }

    /// 从 `initialize` 响应的 `result.capabilities` 里取 RA 报的 legend。
    fn from_capabilities(caps: &Value) -> Self {
        let names = caps
            .get("semanticTokensProvider")
            .and_then(|p| p.get("legend"))
            .and_then(|l| l.get("tokenTypes"))
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>());
        match names {
            Some(n) if !n.is_empty() => Self::from_names(n),
            // 没报 / 报成空 = 无从翻译,保持原样。
            _ => Self::default(),
        }
    }

    /// 单个 RA 索引 → 我们的索引。越界(RA 的 legend 比我们长)也当
    /// 「不着色」,不能让 `map[i]` 整个 panic —— 一个新 token 类型不该把
    /// 整份高亮带崩。
    fn remap_one(&self, ra_index: u32) -> u32 {
        self.map.get(ra_index as usize).copied().unwrap_or(Self::NO_COLOR)
    }

    /// 把 RA 的 delta 数组里每个 token 的**类型**换成我们的下标。
    ///
    /// delta 是 5 个一组 `(Δline, Δstart, len, type, modifiers)`,只动每组
    /// 第 4 个。**长度必须与入参一致** —— 前端按固定步长解码,少一个元素
    /// 就整体错位。尾部不足 5 个的残缺组原样保留。
    fn remap(&self, data: &[u32]) -> Vec<u32> {
        if self.map.is_empty() {
            return data.to_vec();
        }
        let mut out = data.to_vec();
        for i in (0..data.len() / 5 * 5).step_by(5) {
            out[i + 3] = self.remap_one(data[i + 3]);
        }
        out
    }
}

/// 内部请求 id 的起点。前端 id 从 1 起,正常不会到达这里;真到了就拒绝。
pub const INTERNAL_ID_BASE: u64 = 0x4000_0000;

/// `demo/src/lib.rs` 在 RA workspace 里的 URI。必须和 `docker::RA_WORKDIR`
/// 拼出来的一致 —— 有测试钉住。
pub const DOC_URI: &str = "file:///opt/ra/ws/demo/src/lib.rs";

/// 补全项上的展示用注释(来自 LSP `CompletionItem.labelDetails`)。
/// 声明 `labelDetailsSupport` 之后 RA 会把 `(as Trait)` / 别名放这里,
/// 而不是拼进 `label`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionLabelDetails {
    pub detail: Option<String>,
    pub description: Option<String>,
}

/// 一条补全候选。只取前端渲染要用的字段;`kind` 是 LSP 的数字枚举,
/// 原样传,映射成样式是前端的活。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
    pub label_details: Option<CompletionLabelDetails>,
}

/// 一条诊断。位置是 LSP 的 0 基行列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub severity: u8,
    pub message: String,
}

/// 会话推给上层的事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 当前编辑文件(`DOC_URI`)的一次完整诊断集合。
    ///
    /// **空 vec 有意义**:LSP 用空数组表示「该 uri 的诊断清空」,
    /// 上层据此把红线擦掉。
    Diagnostics(Vec<Diagnostic>),
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
    /// `textDocument/diagnostic`(拉取式诊断)的响应,reader 转成
    /// [`Event::Diagnostics`]。
    Diagnostics,
    /// `request_id` 是前端给的 id —— 回显在 `Event::Completed` 里。
    Completion {
        request_id: u32,
    },
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
    /// 内部请求 id,从 [`INTERNAL_ID_BASE`] 单调递增。reader 处理
    /// `workspace/semanticTokens/refresh` 时也要分配,所以共享 `Arc`。
    next_internal_id: Arc<AtomicU64>,
    /// didOpen / didChange 共用的文档版本号。
    version: AtomicU64,
    /// RA 报的 legend,`initialize` 拿到响应后填上。token 转发时用它
    /// 把 RA 的下标翻成 [`TOKEN_TYPES`] 的下标。
    legend: Arc<Mutex<TokenLegend>>,
    /// 最近一次**完整**诊断集合。拉取式诊断在内容没变时会回
    /// `kind: "unchanged"` 且不带 `items`;池里的会话还会被下一条连接
    /// 复用,新连接起来的第一次拉取完全可能命中 unchanged —— 缓存一份,
    /// 这两种情况才有东西可发。
    last_diagnostics: Arc<Mutex<Vec<Diagnostic>>>,
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
        let legend = Arc::new(Mutex::new(TokenLegend::default()));
        let last_diagnostics = Arc::new(Mutex::new(Vec::new()));
        let reader = tokio::spawn(reader_loop(
            Arc::clone(&proc),
            tx.clone(),
            Arc::clone(&alive),
            Arc::clone(&pending),
            Arc::clone(&next_internal_id),
            Arc::clone(&legend),
            Arc::clone(&last_diagnostics),
        ));
        Self {
            proc,
            events_tx: tx,
            legend,
            last_diagnostics,
            pending,
            alive,
            next_internal_id,
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
                    // 声明支持**拉取式**诊断 —— RA 1.10x 把诊断改成了
                    // `textDocument/diagnostic`:`publishDiagnostics` 只在
                    // 工作区加载完时推一次,之后文档怎么改都不再推。不声明
                    // 也不拉的话,波浪线永远停在第一次那份内容上。
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

        // RA 会回它**自己那份** legend(实测是字母序,不是我们上报的那份)。
        // 拿它把后续 token 的类型下标翻译到我们的表上,否则 `pub` / `fn`
        // 会按 RA 的 `keyword`(6) 被前端当 `typeParameter`(6) 染成斜体。
        let legend = resp
            .get("result")
            .and_then(|r| r.get("capabilities"))
            .map(TokenLegend::from_capabilities)
            .unwrap_or_default();
        self.legend
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .map = legend.map;

        self.send_json(&json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {},
        }))
        .await
    }

    /// 打开文档(全文),版本从 1 起,随后主动要一次 token。
    ///
    /// `didOpen` 之后就要请求:RA 不会「打开后自动推 token」,不主动要
    /// 的话用户不敲键盘就永远没有高亮。
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
        .await?;
        self.request_tokens().await?;
        self.request_diagnostics().await
    }

    /// 全文替换(不带 `range`),然后主动要一次 token 与诊断。
    ///
    /// 两者 RA 都不会在改动后自己推:token 要靠请求;诊断在 RA 1.10x
    /// 已经是拉取式的(`publishDiagnostics` 只在工作区加载完时推一次)。
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
        self.request_tokens().await?;
        self.request_diagnostics().await
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

    /// 订阅事件。每次调用都拿到一个独立的接收端 —— 会话在池里跨连接
    /// 复用时,前后两条连接各自订阅,互不影响。
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events_tx.subscribe()
    }

    /// 存活性检查。**必须走 pending 表**:直接读 `RaProcess` 会和后台
    /// reader 抢同一根管道,把 token/诊断帧偷走,或让健康会话被误判死亡。
    /// RA 对未知方法回 `error` 响应 —— 有响应就是活着。
    pub async fn ping(&self, timeout: Duration) -> Result<(), LspError> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .insert(id, Pending::Response(tx));
        let req = json!({
            "jsonrpc": "2.0", "id": id, "method": "r2g/ping", "params": Value::Null
        });
        if let Err(e) = self.send_json(&req).await {
            self.unregister(id);
            return Err(e);
        }
        match tokio::time::timeout(timeout, rx).await {
            // 有响应就活着 —— 错误响应也算。
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err(LspError::Closed),
            Err(_) => {
                self.unregister(id);
                Err(LspError::Timeout)
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// 主动关:杀子进程,并让 reader 在下一轮 `recv` 后收工。
    pub async fn shutdown(&self) {
        self.proc.shutdown().await;
        self.alive.store(false, Ordering::SeqCst);
    }

    /// 主动要一次 `textDocument/semanticTokens/full`。响应由 reader
    /// 转成 [`Event::Tokens`]。
    async fn request_tokens(&self) -> Result<(), LspError> {
        request_tokens_impl(&self.proc, &self.pending, &self.next_internal_id).await
    }

    /// 主动拉一次 `textDocument/diagnostic`。响应由 reader 转成
    /// [`Event::Diagnostics`](拉取式诊断的唯一来源)。
    async fn request_diagnostics(&self) -> Result<(), LspError> {
        request_diagnostics_impl(&self.proc, &self.pending, &self.next_internal_id).await
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

/// 登记 `Pending::Tokens` 并发出 `semanticTokens/full`。
///
/// 抽成自由函数是因为 reader 处理 `workspace/semanticTokens/refresh` 时
/// 也要发一次,而 reader 手里只有 `Arc<RaProcess>` / pending 表 / id 计数器。
async fn request_tokens_impl(
    proc: &RaProcess,
    pending: &Mutex<HashMap<u64, Pending>>,
    next_id: &AtomicU64,
) -> Result<(), LspError> {
    let id = next_id.fetch_add(1, Ordering::SeqCst);
    pending
        .lock()
        .expect("Mutex 不该中毒:持锁段不 panic")
        .insert(id, Pending::Tokens);
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/semanticTokens/full",
        "params": { "textDocument": { "uri": DOC_URI } },
    });
    if let Err(e) = proc.send(&req.to_string()).await {
        pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .remove(&id);
        return Err(LspError::Exec(e.to_string()));
    }
    Ok(())
}

/// 登记 `Pending::Diagnostics` 并发出 `textDocument/diagnostic`。
///
/// 与 [`request_tokens_impl`] 同理,reader 处理 `workspace/diagnostic/refresh`
/// 时也要发一次。**不传 `previousResultId`**:RA 只有拿到上一份的
/// `resultId` 才可能回 `kind: "unchanged"`,而不带 items 的 unchanged
/// 对刚连上来的新客户端毫无意义 —— 池里的会话是跨连接复用的,新连接
/// 手上没有上一份。不带就总能拿到完整集合。
async fn request_diagnostics_impl(
    proc: &RaProcess,
    pending: &Mutex<HashMap<u64, Pending>>,
    next_id: &AtomicU64,
) -> Result<(), LspError> {
    let id = next_id.fetch_add(1, Ordering::SeqCst);
    pending
        .lock()
        .expect("Mutex 不该中毒:持锁段不 panic")
        .insert(id, Pending::Diagnostics);
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/diagnostic",
        "params": { "textDocument": { "uri": DOC_URI } },
    });
    if let Err(e) = proc.send(&req.to_string()).await {
        pending
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .remove(&id);
        return Err(LspError::Exec(e.to_string()));
    }
    Ok(())
}

/// 后台读循环:把 RA 的每一帧分到 pending 表或事件里。
async fn reader_loop(
    proc: Arc<RaProcess>,
    tx: broadcast::Sender<Event>,
    alive: Arc<AtomicBool>,
    pending: Arc<Mutex<HashMap<u64, Pending>>>,
    next_id: Arc<AtomicU64>,
    legend: Arc<Mutex<TokenLegend>>,
    last_diagnostics: Arc<Mutex<Vec<Diagnostic>>>,
) {
    while alive.load(Ordering::SeqCst) {
        match proc.recv(RECV_TIMEOUT).await {
            Ok(Some(raw)) => {
                // 不是 JSON 的帧:忽略。RA 的 stdout 上只该有 LSP 帧。
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    dispatch(v, &proc, &tx, &pending, &next_id, &legend, &last_diagnostics).await;
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

async fn dispatch(
    v: Value,
    proc: &RaProcess,
    tx: &broadcast::Sender<Event>,
    pending: &Mutex<HashMap<u64, Pending>>,
    next_id: &AtomicU64,
    legend: &Mutex<TokenLegend>,
    last_diagnostics: &Mutex<Vec<Diagnostic>>,
) {
    // 带 method 的是通知或服务端发来的请求。
    if let Some(method) = v.get("method").and_then(Value::as_str) {
        if method == "textDocument/publishDiagnostics" {
            emit_diagnostics(&v, tx, last_diagnostics);
            return;
        }
        // RA 拉取式诊断的「有东西变了」信号。它**是通知**(不带 id,
        // 不需要回响应),收到就主动拉一次。RA 1.10x 只在工作区加载完
        // 时推 publishDiagnostics,之后的诊断全靠这条链路。
        if method == "workspace/diagnostic/refresh" {
            // 规范里它是通知;万一带了 id 也照下面的规矩回一个空结果。
            if let Some(id) = v.get("id") {
                let resp = json!({ "jsonrpc": "2.0", "id": id.clone(), "result": Value::Null });
                if let Err(e) = proc.send(&resp.to_string()).await {
                    tracing::warn!("回复 RA 的诊断刷新请求失败:{e}");
                }
            }
            if let Err(e) = request_diagnostics_impl(proc, pending, next_id).await {
                tracing::warn!("刷新诊断失败:{e}");
            }
            return;
        }
        // 服务端**请求**(带 id)必须回响应:静默丢弃会让 RA 以为
        // 客户端挂了,后续分析可能停摆。请求的参数我们都不需要,
        // 统一回 `result: null`。
        if let Some(id) = v.get("id") {
            let resp = json!({
                "jsonrpc": "2.0",
                "id": id.clone(),
                "result": Value::Null,
            });
            if let Err(e) = proc.send(&resp.to_string()).await {
                tracing::warn!("回复 RA 的服务端请求失败:{e}");
            }
            // RA 要求客户端重新拉 token(冷启动时第一次拿到的可能
            // 不完整)。这是高亮能补齐的关键一步。
            if method == "workspace/semanticTokens/refresh"
                && let Err(e) = request_tokens_impl(proc, pending, next_id).await
            {
                tracing::warn!("刷新 semanticTokens 失败:{e}");
            }
        }
        // 其余通知(window/logMessage、$/progress 等)忽略。
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
            // `delta` 数组整体按 `Vec<u32>` 解析(逐元素 `filter_map` 会静默
            // 丢元素/截断),随后**只把每个 token 的类型下标**从 RA 的
            // legend 翻到我们的 `TOKEN_TYPES`。坐标、修饰符原样不动 ——
            // 拆成 (start, len, type, modifiers) 是前端 CodeMirror 的活。
            let Some(data) = v.get("result").and_then(|r| r.get("data")) else {
                tracing::warn!("semanticTokens/full 响应里没有 data,丢弃");
                return;
            };
            match serde_json::from_value::<Vec<u32>>(data.clone()) {
                Ok(data) => {
                    let data = legend
                        .lock()
                        .expect("Mutex 不该中毒:持锁段不 panic")
                        .remap(&data);
                    let _ = tx.send(Event::Tokens(data));
                }
                Err(e) => {
                    // 不能发一个空 Tokens 假装正常 —— 前端会据此把
                    // 整份高亮擦掉。
                    tracing::warn!("semanticTokens/full 的 data 解析失败,丢弃:{e}");
                }
            }
        }
        Pending::Diagnostics => {
            let mut cache = last_diagnostics
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic");
            match parse_diagnostic_result(&v) {
                // 完整集合:换掉缓存再发。
                Some(Some(items)) => {
                    *cache = items.clone();
                    let _ = tx.send(Event::Diagnostics(items));
                }
                // `unchanged`:新连接手上什么都没有,发缓存里的那一份。
                Some(None) => {
                    let _ = tx.send(Event::Diagnostics(cache.clone()));
                }
                None => tracing::warn!("textDocument/diagnostic 响应不合约,丢弃"),
            }
        }
        Pending::Completion { request_id } => {
            let items = parse_completion_items(v.get("result"));
            let _ = tx.send(Event::Completed { request_id, items });
        }
    }
}

fn emit_diagnostics(v: &Value, tx: &broadcast::Sender<Event>, last: &Mutex<Vec<Diagnostic>>) {
    let Some(params) = v.get("params") else {
        return;
    };
    // 工作区里还有 lib / lib-internal / Cargo.toml 的诊断;只认当前
    // 编辑文件的,否则前端会把别处的问题画在这份源码上。
    if params.get("uri").and_then(Value::as_str) != Some(DOC_URI) {
        return;
    }
    let Some(list) = params.get("diagnostics").and_then(Value::as_array) else {
        return;
    };
    // 空数组也要发:它表示「该 uri 的诊断清空」,上层据此擦红线。
    let diagnostics: Vec<Diagnostic> = list.iter().filter_map(diagnostic_from).collect();
    // 推送和拉取共享同一份缓存:之后某次拉取回 `unchanged` 时,
    // 要发的就是这一份。
    *last.lock().expect("Mutex 不该中毒:持锁段不 panic") = diagnostics.clone();
    let _ = tx.send(Event::Diagnostics(diagnostics));
}

/// `textDocument/diagnostic` 的 `result` 有几种形状:
/// - `{ "kind": "full", "items": [...] }` —— 完整集合
/// - `{ "kind": "unchanged" }` —— 与上一份相同,**不带 items**
/// - 裸数组 —— 老形状,按完整集合处理
///
/// 返回值:`Some(Some(items))` = 完整集合,`Some(None)` = unchanged,
/// `None` = 形状不认识。注意 unchanged 与「解析失败」必须分开 ——
/// 把 unchanged 当失败丢掉,新连接就永远收不到诊断。
fn parse_diagnostic_result(v: &Value) -> Option<Option<Vec<Diagnostic>>> {
    let result = v.get("result")?;
    if result.get("kind").and_then(Value::as_str) == Some("unchanged") {
        return Some(None);
    }
    let list = result
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| result.as_array())?;
    Some(Some(list.iter().filter_map(diagnostic_from).collect()))
}

fn diagnostic_from(d: &Value) -> Option<Diagnostic> {
    let range = d.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some(Diagnostic {
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
                label_details: it.get("labelDetails").map(|d| CompletionLabelDetails {
                    detail: d.get("detail").and_then(Value::as_str).map(str::to_string),
                    description: d
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                }),
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
    async fn initialize_then_did_open_then_did_change() {
        let (fake, session) = setup().await;
        let tx = fake.sender();

        // initialize 要等响应:后台看到请求写出去之后再回帧。
        let feeder = tokio::spawn({
            let fake = Arc::clone(&fake);
            async move {
                let _ = fake.wait_until_written("\"method\":\"initialize\"").await;
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
        assert_eq!(
            init["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["labelDetailsSupport"],
            true,
            "不声明这个 RA 会把 `(as Trait)` 拼进 label,选中后插进代码"
        );
        // tokenTypes 的**顺序**必须整体钉死:前端按同一顺序把索引映射成
        // 样式 class,只钉首尾/长度挡不住中间 21 项被重排。
        assert_eq!(
            init["params"]["capabilities"]["textDocument"]["semanticTokens"]["tokenTypes"],
            serde_json::json!(TOKEN_TYPES),
            "tokenTypes 顺序必须与前端一致"
        );

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
        let mut events = session.subscribe();
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
            Event::Diagnostics(vec![Diagnostic {
                line: 3,
                column: 7,
                end_line: 3,
                end_column: 12,
                severity: 2,
                message: "unresolved import".into(),
            }])
        );
    }

    /// 别的文件(工作区里的 lib / Cargo.toml)的诊断不能冒充当前文件;
    /// 空数组必须发出来 —— 否则用户修好错误后红线永远不消失。
    #[tokio::test]
    async fn diagnostics_are_filtered_by_uri_and_empty_clears() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        let tx = fake.sender();

        let foreign = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": "file:///opt/ra/ws/Cargo.toml",
                "diagnostics": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 1 },
                    },
                    "severity": 1,
                    "message": "foreign",
                }],
            },
        });
        tx.send(frame::encode(&foreign.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        assert!(
            tokio::time::timeout(Duration::from_millis(300), events.recv())
                .await
                .is_err(),
            "别的 uri 的诊断不该发事件"
        );

        let cleared = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": DOC_URI, "diagnostics": [] },
        });
        tx.send(frame::encode(&cleared.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        assert_eq!(
            next_event(&mut events).await,
            Event::Diagnostics(vec![]),
            "空数组 = 诊断清空,必须发一个空 vec"
        );
    }

    /// RA 1.10x 的诊断是**拉取式**的:文档改动后它不再推
    /// `publishDiagnostics`,要自己发 `textDocument/diagnostic`。
    /// 不拉的话波浪线永远停在第一次那份内容上。
    #[tokio::test]
    async fn did_change_pulls_diagnostics() {
        let (fake, session) = setup().await;
        session
            .did_change("fn main() {}")
            .await
            .expect("didChange 该成功");

        let bodies = fake.written_bodies();
        let pull = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/diagnostic")
            .unwrap_or_else(|| panic!("didChange 之后要拉一次诊断:{bodies:?}"));
        assert_eq!(pull["params"]["textDocument"]["uri"], DOC_URI);
        assert!(
            pull.get("previousResultId").is_none(),
            "不带 previousResultId 才总会拿到完整集合:{pull}"
        );
    }

    /// 拉取式诊断的 `full` 响应要变成 [`Event::Diagnostics`]。
    #[tokio::test]
    async fn pulled_diagnostics_become_an_event() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        session.did_change("fn main() {}").await.expect("didChange 该成功");
        let id = fake
            .wait_until_written("\"method\":\"textDocument/diagnostic\"")
            .await
            .expect("拉取请求该带 id");

        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "kind": "full",
                "items": [{
                    "range": {
                        "start": { "line": 0, "character": 9 },
                        "end": { "line": 0, "character": 13 },
                    },
                    "severity": 1,
                    "message": "mismatched types",
                }],
            },
        });
        fake.sender()
            .send(frame::encode(&resp.to_string()))
            .expect("UnboundedSender 不该 send 失败");

        assert_eq!(
            next_event(&mut events).await,
            Event::Diagnostics(vec![Diagnostic {
                line: 0,
                column: 9,
                end_line: 0,
                end_column: 13,
                severity: 1,
                message: "mismatched types".into(),
            }])
        );
    }

    /// `kind: "unchanged"` 不带 items。池里的会话跨连接复用,新连接
    /// 起来时手上什么都没有 —— 必须回放缓存,否则它就永远看不到诊断。
    #[tokio::test]
    async fn unchanged_diagnostics_replay_the_cached_set() {
        let (fake, session) = setup().await;
        let mut first = session.subscribe();

        // 第一份完整集合:写进缓存。
        let id = {
            session.did_change("fn main() {}").await.expect("didChange 该成功");
            fake.wait_until_written("\"method\":\"textDocument/diagnostic\"")
                .await
                .expect("拉取请求该带 id")
        };
        let full = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "kind": "full",
                "items": [{
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 8 },
                    },
                    "severity": 1,
                    "message": "no method named `foo`",
                }],
            },
        });
        fake.sender()
            .send(frame::encode(&full.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        let cached = Event::Diagnostics(vec![Diagnostic {
            line: 2,
            column: 4,
            end_line: 2,
            end_column: 8,
            severity: 1,
            message: "no method named `foo`".into(),
        }]);
        assert_eq!(next_event(&mut first).await, cached);

        // 第二个「客户端」(新订阅者)拉取时 RA 回 unchanged。
        let mut second = session.subscribe();
        session.did_change("fn main() {}").await.expect("didChange 该成功");
        let id = fake
            .wait_until_written("\"method\":\"textDocument/diagnostic\"")
            .await
            .expect("拉取请求该带 id");
        let unchanged = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "kind": "unchanged" },
        });
        fake.sender()
            .send(frame::encode(&unchanged.to_string()))
            .expect("UnboundedSender 不该 send 失败");

        assert_eq!(
            next_event(&mut second).await,
            cached,
            "unchanged 要回放缓存给新订阅者"
        );
    }

    /// RA 发 `workspace/diagnostic/refresh`(通知,不带 id)时要再拉一次 ——
    /// 这是它在工作区状态变化后唯一的提示方式。
    #[tokio::test]
    async fn diagnostic_refresh_notification_pulls_again() {
        let (fake, session) = setup().await;
        let _events = session.subscribe();

        let note = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/diagnostic/refresh",
            "params": null,
        });
        fake.sender()
            .send(frame::encode(&note.to_string()))
            .expect("UnboundedSender 不该 send 失败");

        let _ = fake
            .wait_until_written("\"method\":\"textDocument/diagnostic\"")
            .await;
    }

    /// `didOpen` 之后就要主动要一次 token —— RA 不会自己推,不要的话
    /// 用户不敲键盘就永远没有高亮。
    #[tokio::test]
    async fn did_open_requests_semantic_tokens() {
        let (fake, session) = setup().await;
        session
            .did_open("fn main() {}")
            .await
            .expect("didOpen 该成功");

        let bodies = fake.written_bodies();
        let req = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/semanticTokens/full")
            .unwrap_or_else(|| panic!("didOpen 之后就该请求 token:{bodies:?}"));
        assert_eq!(
            req["id"].as_u64(),
            Some(INTERNAL_ID_BASE),
            "token 请求要用内部 id"
        );
        assert_eq!(req["params"]["textDocument"]["uri"], DOC_URI);
    }

    #[tokio::test]
    async fn semantic_tokens_pass_through_untouched() {
        // 服务端不拆 delta:Event::Tokens 拿到的就是 RA 给的那串
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        // didChange 会主动要 semanticTokens/full;这是会话的第一个内部
        // 请求,id = INTERNAL_ID_BASE。
        session
            .did_change("fn main() {}")
            .await
            .expect("didChange 该成功");

        // 先确认请求真的写出去了(否则只测「塞响应→收事件」会掩盖
        // 根本没发请求的情况),并从请求里取 id。
        let bodies = fake.written_bodies();
        let req = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/semanticTokens/full")
            .unwrap_or_else(|| panic!("didChange 之后该请求 token:{bodies:?}"));
        let id = req["id"].as_u64().expect("内部请求该有数字 id");
        assert_eq!(req["params"]["textDocument"]["uri"], DOC_URI);

        let data: Vec<u32> = vec![0, 0, 3, 4, 0, 1, 1, 5, 1, 0, 2, 3, 1, 22, 0];
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
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

    /// `data` 里混了非 u32 元素时不能静默丢元素、更不能假装发一个空
    /// Tokens —— 那会让前端把整份高亮擦掉。
    #[tokio::test]
    async fn malformed_token_data_emits_no_event() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
        session
            .did_change("fn main() {}")
            .await
            .expect("didChange 该成功");
        let bodies = fake.written_bodies();
        let id = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/semanticTokens/full")
            .expect("didChange 之后该请求 token")["id"]
            .as_u64()
            .expect("内部请求该有数字 id");

        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "data": [0, 1, "bad"] },
        });
        fake.sender()
            .send(frame::encode(&resp.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        assert!(
            tokio::time::timeout(Duration::from_millis(300), events.recv())
                .await
                .is_err(),
            "解析失败不能发 Event::Tokens"
        );
    }

    /// RA 的 legend 是**它自己排的序**,不是我们 `initialize` 里报的那份。
    /// 实测 RA 1.101 回的是字母序:`keyword` 在第 6 位;我们表里第 6 位是
    /// `typeParameter` —— 按自己的表去解释 RA 的索引,`pub` / `fn` 就会
    /// 被染成「泛型参数」的斜体。映射必须按**名字**做。
    #[test]
    fn legend_maps_ra_alphabetical_names_onto_our_table() {
        // 这份是 RA 实测回来的顺序(截取到我们有映射的那一段)。
        let ra: Vec<&str> = vec![
            "comment", "decorator", "enumMember", "enum", "function", "interface", "keyword",
            "macro", "method", "namespace", "number", "operator", "parameter", "property",
            "string", "struct", "typeParameter", "variable", "type",
        ];
        let legend = TokenLegend::from_names(ra);
        assert_eq!(legend.remap_one(6), 15, "keyword 在我们表里是 15");
        assert_eq!(legend.remap_one(4), 12, "function 在我们表里是 12");
        assert_eq!(legend.remap_one(0), 17, "comment 在我们表里是 17");
        assert_eq!(legend.remap_one(10), 19, "number 在我们表里是 19");
    }

    /// RA 的标点类 token(`brace` / `comma` / `lifetime` …)不在我们表里。
    /// 它们必须变成「前端跳过」的哨兵值,**同时不能打乱 5 个一组的步长** ——
    /// 错位一个元素,后面所有 token 的类型全错。
    #[test]
    fn names_outside_our_table_become_a_skip_sentinel_without_shifting() {
        let legend = TokenLegend::from_names(vec!["comment", "keyword", "brace", "lifetime"]);
        assert_eq!(legend.remap_one(2), TokenLegend::NO_COLOR);
        assert_eq!(legend.remap_one(3), TokenLegend::NO_COLOR);
        assert_eq!(legend.remap_one(1), 15);
        // 三组完整的 token,类型位分别取 `brace` / `keyword` / `lifetime`。
        let data = vec![0, 0, 3, 2, 0, 0, 3, 3, 1, 0, 0, 5, 3, 3, 0];
        assert_eq!(
            legend.remap(&data),
            vec![
                0,
                0,
                3,
                TokenLegend::NO_COLOR,
                0,
                0,
                3,
                3,
                15,
                0,
                0,
                5,
                3,
                TokenLegend::NO_COLOR,
                0
            ]
        );
        // 长度必须不变:前端按 5 个一组解码,少一个就整体错位。
        assert_eq!(legend.remap(&data).len(), data.len());
    }

    /// 尾部的残缺组(不足 5 个)不能被改写 —— 前端解码时本来就会丢掉,
    /// 这里也别碰它。
    #[test]
    fn a_trailing_partial_group_is_left_alone() {
        let legend = TokenLegend::from_names(vec!["comment", "keyword"]);
        assert_eq!(legend.remap(&[0, 1, 2, 1, 0, 9, 9]), vec![0, 1, 2, 15, 0, 9, 9]);
    }

    /// RA 没报 legend(老版本 / 能力缺失)时按原样透传 —— 此时索引由前端
    /// 那张表解释,和这次改动之前的行为一致。
    #[test]
    fn an_empty_legend_is_the_identity() {
        let legend = TokenLegend::from_names(Vec::<String>::new());
        let data = vec![0, 0, 3, 6, 0];
        assert_eq!(legend.remap(&data), data);
    }

    /// 端到端:initialize 的响应里带 RA 那份 legend,之后回来的 token
    /// 里 `type` 字段(第 4 个)已经被翻译成我们表里的下标。
    #[tokio::test]
    async fn tokens_are_remapped_through_the_legend_from_initialize() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();

        let feeder = {
            let fake = Arc::clone(&fake);
            tokio::spawn(async move {
                fake.wait_until_written("\"method\":\"initialize\"").await;
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": INTERNAL_ID_BASE,
                    "result": { "capabilities": { "semanticTokensProvider": {
                        "legend": { "tokenTypes": ["comment", "keyword"] }
                    }}},
                });
                fake.sender()
                    .send(frame::encode(&resp.to_string()))
                    .expect("回 initialize 响应");
                fake.wait_until_written("textDocument/semanticTokens/full").await;
                let bodies = fake.written_bodies();
                let id = bodies
                    .iter()
                    .find(|v| v["method"] == "textDocument/semanticTokens/full")
                    .map(|v| v["id"].as_u64().unwrap())
                    .unwrap();
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "data": [0, 0, 3, 1, 0] },
                });
                fake.sender()
                    .send(frame::encode(&resp.to_string()))
                    .expect("回 token 响应");
            })
        };

        session.initialize().await.expect("initialize 该成功");
        // didChange 会触发 token 请求(会话 booted 后灌过一次占位源码)。
        let _ = session.did_change("fn main() {}").await;
        let _ = feeder.await;

        match next_event(&mut events).await {
            Event::Tokens(got) => assert_eq!(got[3], 15, "RA 的 keyword(1) 要翻成我们的 15"),
            other => panic!("期望 Event::Tokens,实际:{other:?}"),
        }
    }

    /// 服务端发来的请求必须回一条 `result: null`(静默丢弃会让 RA
    /// 以为客户端挂了);`workspace/semanticTokens/refresh` 还要重新
    /// 请求一次 token —— 冷启动时第一次拿到的可能不完整。
    #[tokio::test]
    async fn server_requests_are_answered_and_refresh_refetches_tokens() {
        let (fake, session) = setup().await;
        let _events = session.subscribe();
        let tx = fake.sender();

        let refresh = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "workspace/semanticTokens/refresh",
            "params": null,
        });
        tx.send(frame::encode(&refresh.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        let _ = fake.wait_until_written("\"id\":42").await;

        let bodies = fake.written_bodies();
        let resp = bodies
            .iter()
            .find(|v| v["id"].as_u64() == Some(42) && v.get("method").is_none())
            .unwrap_or_else(|| panic!("刷新请求必须回一条响应:{bodies:?}"));
        assert_eq!(resp["result"], serde_json::Value::Null);
        let token_req = bodies
            .iter()
            .find(|v| v["method"] == "textDocument/semanticTokens/full")
            .unwrap_or_else(|| panic!("刷新之后要重新请求 token:{bodies:?}"));
        assert_eq!(
            token_req["id"].as_u64(),
            Some(INTERNAL_ID_BASE),
            "重新请求 token 要用新的内部 id"
        );
        assert_eq!(token_req["params"]["textDocument"]["uri"], DOC_URI);

        // 其它服务端请求同样要回,不能静默丢。
        let register = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "client/registerCapability",
            "params": {},
        });
        tx.send(frame::encode(&register.to_string()))
            .expect("UnboundedSender 不该 send 失败");
        let _ = fake.wait_until_written("\"id\":9").await;

        let bodies = fake.written_bodies();
        let resp = bodies
            .iter()
            .find(|v| v["id"].as_u64() == Some(9) && v.get("method").is_none())
            .expect("未知服务端请求也要回响应");
        assert_eq!(resp["result"], serde_json::Value::Null);
    }

    /// RA 在 `initialize` 等待期间死掉:reader 清空 pending 后 oneshot
    /// 立刻断,`initialize` 应当秒级返回 `Closed`,而不是等满 30s。
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
    async fn completion_request_uses_the_callers_id() {
        let (fake, session) = setup().await;
        let mut events = session.subscribe();
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
                        label_details: None,
                    },
                    CompletionItem {
                        label: "advance".into(),
                        kind: Some(3),
                        detail: None,
                        label_details: None,
                    },
                ],
            }
        );
    }

    /// `ping` 必须走 pending 表,而不是直接读 `RaProcess` —— 后者会和
    /// 后台 reader 抢同一根管道。这里让 reader 正常吃响应,ping 只负责
    /// 登记 + 等 oneshot:能拿到就说明它没有去抢管道。
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
}
