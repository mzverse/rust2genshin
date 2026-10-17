//! WebSocket:一条连接、一问一答式地把整条链路走完。
//!
//! 一条连接同时只跑一个任务 —— 不然 `cancel` 取消的是哪一个?排队期间
//! **不阻塞**读循环(额度交接在 spawn 出去的任务里等),所以排队时断开、
//! 点取消都还能被看见。
//!
//! 断开 = 取消。任务被 `abort` 掉,它手里的 `Ticket` 随之 drop,该 IP 的
//! 占用和并发额度在同一步还回去 —— 泄漏的路径只有"进程死掉"这一条,
//! 那条路上操作系统会把容器一起收走。

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

use crate::{
    docker::Stream,
    job::JobMeta,
    lsp::{pool::Acquire, session::Event},
    protocol::{ClientMsg, ErrorCode, MAX_SOURCE_BYTES, PROTOCOL_VERSION, SOURCE_FILE, ServerMsg},
    runner::{JobSpec, RunnerError},
    server::{AppState, client_key_of, origin_allowed},
};

pub async fn ws_upgrade(
    State(st): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Response {
    let origin = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok());
    // CORS 管不着 WebSocket:浏览器对 ws:// 不预检,也不看 ACAO。
    // 不查这一句,任何页面都能拿用户的浏览器当跳板。
    if !origin_allowed(origin, &st.cfg.allowed_origins) {
        return (StatusCode::FORBIDDEN, "来源不在白名单里").into_response();
    }
    // 必须用 `client_key_of`,不是 `client_ip(..).to_string()` —— 后者漏掉
    // IPv6 /64 归并,一个 /64 里换地址就绕过了"每 IP 一个"。
    let ip = client_key_of(&headers, peer, st.cfg.trust_proxy);
    // 排队时最长要等几十秒,握手完就得能持续收发。
    upgrade
        .max_message_size(MAX_SOURCE_BYTES + 4096)
        .on_upgrade(move |socket| run_socket(socket, st, ip))
}

/// 失败转成给用户看的消息。**没有任何一条路径吞掉失败** ——
/// 退出码 0 但没产物也走这里(见 runner::verdict)。
pub fn describe_error(e: &RunnerError, task: &str) -> ServerMsg {
    let (exit_code, message) = match e {
        RunnerError::NoArtifact { code } => (
            *code,
            "编译没有产出 .gia。容器输出见上面的日志。".to_string(),
        ),
        RunnerError::Timeout(t) => (
            None,
            format!("超过 {} 秒还没编完,已经结束。把图写小一点,或者拆成几个复合节点。", t.as_secs()),
        ),
        RunnerError::Spawn(why) => (None, format!("起不了容器:{why}")),
        RunnerError::Decode(why) => (None, format!("读容器输出失败:{why}")),
    };
    ServerMsg::Failed {
        task: task.to_string(),
        exit_code,
        message,
    }
}

pub async fn run_socket(socket: WebSocket, state: Arc<AppState>, ip: String) {
    let bake = state.baker.status();
    let hello = ServerMsg::Hello {
        protocol: PROTOCOL_VERSION,
        image_commit: bake.serving.clone(),
        file: SOURCE_FILE.to_string(),
        max_jobs: state.cfg.max_jobs,
        max_queue: state.cfg.max_queue,
        // 服务器节点图的目标。由 `protocol::TARGETS` 生成 —— 加目标只改
        // 那张表,不是在这里 push,也不是改协议。
        targets: crate::protocol::TARGETS.iter().map(|s| s.to_string()).collect(),
        // 装了 RA 运行时才有;DISABLED 时前端退回纯编译界面。
        lsp_supported: state.lsp.is_some(),
    };
    let (mut sink, mut stream) = socket.split();
    if send(&mut sink, &hello).await.is_err() {
        return;
    }

    // 任务往这条通道里塞消息,不直接碰 socket —— 于是"任务在跑"和
    // "读循环还活着"是两件事,排队期间照样能看见断开和 cancel。
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    // 任务名和 handle 一起存:`cancel` 要把 task 原样回给客户端,光有
    // handle 是 abort 不出名字来的。
    let mut job: Option<(String, JoinHandle<()>)> = None;
    // 这条连接手里的 RA 会话。断开时必须还回池 —— RA 常驻 839 MB,
    // 不放回去就等于一直挂在容器里。
    let mut lsp_conn: Option<LspConn> = None;

    // 空闲保活。前面 nginx 的 stream 模块按「最后一次有字节」掐空闲连接
    // (`proxy_timeout`,部署上是 60s),而排队等镜像的那几十秒里一条业务消息
    // 都没有 —— 不主动出声连接会被悄悄关掉,`finished` 连同 .gia 下载链接一起
    // 丢在半路。ping/pong 由 WebSocket 协议本身保证对端会回,浏览器自动就回,
    // 所以这里不需要自己处理 Pong 分支。
    //
    // 周期必须**短于**上游那个超时,留出余量:默认 25s vs 上游 60s。
    let mut beat = tokio::time::interval(state.cfg.keepalive);
    // interval 的第一次 tick 是立即触发的,那样一连上就发一个没必要的 ping。
    beat.tick().await;

    loop {
        tokio::select! {
            _ = beat.tick() => {
                // Ping 帧不进 `send`:那个函数发的是 ServerMsg 的 JSON。
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
            incoming = stream.next() => {
                match incoming {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Err(e) => {
                                if send(&mut sink, &bad_request(format!("看不懂的消息:{e}"))).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMsg::Cancel {}) => {
                                // 已经跑完的任务不算「可取消」:它的 finished/failed
                                // 就在通道里排着,这里要是 abort 后再排空通道,会把
                                // 一条成功的结果吞掉。让它照常发出去,槽位也一并收回。
                                if job.as_ref().is_some_and(|(_, h)| h.is_finished()) {
                                    job = None;
                                    continue;
                                }
                                if let Some((task, h)) = job.take() {
                                    // abort → 任务手里的 Ticket 与 ImageGuard 一起 drop
                                    // → 额度、IP 占用、镜像引用计数一步还清。
                                    h.abort();
                                    let _ = h.await;
                                    // abort 之后不可能再有新消息进来(sender 随任务一起
                                    // drop 了),但**已经进去的**还在:排队→开始那一瞬的
                                    // `started`、临死前几行日志。先排空再回 cancelled,
                                    // 否则客户端可能先收到 cancelled、又收到 started,
                                    // 界面重新跳回「编译中…」。
                                    while rx.try_recv().is_ok() {}
                                    // 前端等的就是这条:没有它,界面永远停在「编译中…」,
                                    // 取消按钮也一直亮着。
                                    if send(&mut sink, &ServerMsg::Cancelled { task }).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Ok(ClientMsg::Submit { source, target }) => {
                                // `Error{Internal}` 这类非终结消息发出来之后,spawn 出去
                                // 的任务可能还在 drop `ticket`/`guard` 的尾巴上 —— 那时
                                // 读循环里 `is_finished()` 还没翻成 true,槽位仍是 `Some`。
                                // `is_finished()` 是单调的:一旦 true 就不会回退,所以这里
                                // 先按它把已结束的槽位收回来,再判断"已有任务在跑",不受
                                // 调度时序影响。
                                if job.as_ref().is_some_and(|(_, h)| h.is_finished()) {
                                    job = None;
                                }
                                if job.is_some() {
                                    if send(&mut sink, &bad_request("已有任务在跑,先发 cancel")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                }
                                match start(&state, &ip, source, target, &tx) {
                                    Err(m) => {
                                        if send(&mut sink, &m).await.is_err() {
                                            break;
                                        }
                                    }
                                    Ok((task, handle)) => job = Some((task, handle)),
                                }
                            }
                            Ok(ClientMsg::LspOpen {}) => {
                                // 幂等:重复 open 不再取一个会话。
                                if lsp_conn.is_some() {
                                    continue;
                                }
                                let Some(svc) = state.lsp.clone() else {
                                    if send(&mut sink, &err(ErrorCode::Internal, "代码分析功能已关闭")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                };
                                let acq =
                                    match tokio::time::timeout(state.cfg.lsp_boot_timeout, svc.acquire()).await {
                                        Ok(Ok(a)) => a,
                                        _ => {
                                            if send(&mut sink, &err(ErrorCode::Internal, "分析器起不来,稍后再试")).await.is_err() {
                                                break;
                                            }
                                            continue;
                                        }
                                    };
                                // 不在这里 didOpen:待命会话补池时已经 didOpen
                                // 过占位内容,同一个 URI 再 didOpen 是 LSP
                                // 协议违规。用户源码走 LspChange 的 didChange。
                                let mut events = acq.session().subscribe();
                                let ftx = tx.clone();
                                let forwarding = tokio::spawn(async move {
                                    loop {
                                        match events.recv().await {
                                            Ok(Event::Tokens(data)) => {
                                                if ftx.send(ServerMsg::LspTokens { data }).is_err() {
                                                    break;
                                                }
                                            }
                                            Ok(Event::Diagnostics(list)) => {
                                                for d in list {
                                                    let m = ServerMsg::LspDiagnostic {
                                                        line: d.line,
                                                        column: d.column,
                                                        end_line: d.end_line,
                                                        end_column: d.end_column,
                                                        severity: d.severity,
                                                        message: d.message,
                                                    };
                                                    if ftx.send(m).is_err() {
                                                        return;
                                                    }
                                                }
                                            }
                                            Ok(Event::Completed { request_id, items }) => {
                                                let items = items
                                                    .into_iter()
                                                    .map(|i| crate::protocol::CompletionItem {
                                                        label: i.label,
                                                        kind: i.kind,
                                                        detail: i.detail,
                                                    })
                                                    .collect();
                                                if ftx.send(ServerMsg::LspCompletion { request_id, items }).is_err() {
                                                    break;
                                                }
                                            }
                                            // 慢消费者丢几条不值得断连;会话关闭才收工。
                                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                            Err(broadcast::error::RecvError::Closed) => break,
                                        }
                                    }
                                });
                                lsp_conn = Some(LspConn { acq, forwarding });
                                if send(&mut sink, &ServerMsg::LspReady { file: SOURCE_FILE.to_string() }).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMsg::LspChange { source }) => {
                                // 不把 `&LspConn` 借到 await 对面去:`Handle`
                                // 里的销毁钩子是 `FnOnce`(非 Sync),借它会让
                                // 整个 `run_socket` future 变成 !Send。
                                let Some((container, session)) = lsp_conn.as_ref().map(|c| {
                                    (c.acq.container().to_string(), Arc::clone(c.acq.session()))
                                }) else {
                                    if send(&mut sink, &bad_request("先发 lsp_open")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                };
                                let Some(svc) = state.lsp.clone() else {
                                    if send(&mut sink, &err(ErrorCode::Internal, "代码分析功能已关闭")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                };
                                // 先写盘再 didChange:否则 RA 可能先看到
                                // didChange、再从磁盘读到旧内容。
                                if let Err(e) = svc.write_source(&container, &source).await {
                                    if send(&mut sink, &err(ErrorCode::Internal, e)).await.is_err() {
                                        break;
                                    }
                                    continue;
                                }
                                if let Err(e) = session.did_change(&source).await
                                    && send(&mut sink, &err(ErrorCode::Internal, format!("分析器没接住这次修改:{e}"))).await.is_err()
                                {
                                    break;
                                }
                            }
                            Ok(ClientMsg::LspCompletion { request_id, line, column }) => {
                                let Some(session) =
                                    lsp_conn.as_ref().map(|c| Arc::clone(c.acq.session()))
                                else {
                                    if send(&mut sink, &bad_request("先发 lsp_open")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                };
                                if let Err(e) = session.request_completion(request_id, line, column).await
                                    && send(&mut sink, &err(ErrorCode::Internal, format!("补全请求失败:{e}"))).await.is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => {} // 二进制 / ping / pong:不收,也不回
                }
            }
            out = rx.recv() => {
                match out {
                    None => break,
                    Some(m) => {
                        let terminal = matches!(m, ServerMsg::Finished { .. } | ServerMsg::Failed { .. });
                        // 消息的终结性与槽位的可复用性**不是同一个集合**:
                        // `Error{Internal}`(建目录 / 写 meta 失败)也意味着那个
                        // 任务已经结束、额度也还了,但消息不是 Finished/Failed。
                        // 只看 terminal 的话槽位会留着,下一次提交被误判成
                        // "已有任务在跑"。handle 结束即视为槽位可复用。
                        let done = terminal || job.as_ref().is_some_and(|(_, h)| h.is_finished());
                        if send(&mut sink, &m).await.is_err() {
                            break;
                        }
                        if done {
                            job = None;
                        }
                    }
                }
            }
        }
    }

    // 每一条离开路径都走这里:断开、协议错、任务收尾。
    if let Some((_, h)) = job.take() {
        h.abort();
        let _ = h.await;
    }
    // RA 会话同样要还。**会话死就销毁,活着回池** —— 死会话放回池只会
    // 让下一个请求再用一次坏 handle。
    if let Some(c) = lsp_conn.take() {
        c.forwarding.abort();
        let alive = c.acq.session().is_alive();
        c.acq.release(alive).await;
    }
}

/// 一条连接持有的 RA 会话,以及把事件转成 `ServerMsg` 的转发任务。
struct LspConn {
    acq: Acquire,
    forwarding: JoinHandle<()>,
}

async fn send<S>(sink: &mut S, m: &ServerMsg) -> Result<(), axum::Error>
where
    S: futures_util::Sink<Message, Error = axum::Error> + Unpin,
{
    let json = serde_json::to_string(m).expect("ServerMsg 一定能序列化");
    sink.send(Message::Text(json.into())).await
}

fn bad_request(why: impl Into<String>) -> ServerMsg {
    ServerMsg::Error {
        code: ErrorCode::BadRequest,
        message: why.into(),
    }
}

fn err(code: ErrorCode, why: impl Into<String>) -> ServerMsg {
    ServerMsg::Error {
        code,
        message: why.into(),
    }
}

/// 提交内容的校验。**纯函数**,和 `start` 分开是为了让它能被单测直接调 ——
/// 这几条判断决定了"什么该起容器",不该只能通过开一条 WebSocket 去验。
///
/// 位置也很要紧:它在抢额度**之前**,所以一个空字符串既不会占住提交者的
/// IP 排队位,也不会浪费一个并发位。
pub fn validate_submit(source: &str, target: Option<&str>) -> Result<(), ServerMsg> {
    if source.trim().is_empty() {
        return Err(bad_request("源码是空的"));
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(bad_request(format!(
            "源码 {} 字节,超过 {} 字节上限",
            source.len(),
            MAX_SOURCE_BYTES
        )));
    }
    if let Some(t) = target
        && !crate::protocol::is_valid_target(t)
    {
        return Err(bad_request(format!(
            "不认识的目标 {t},目前只有 {}",
            crate::protocol::TARGETS.join(" / ")
        )));
    }
    Ok(())
}

/// 校验 → 抢额度 → 起任务。返回 `Err(ServerMsg::Error)` 时**什么都没占**。
fn start(
    state: &Arc<AppState>,
    ip: &str,
    source: String,
    target: Option<String>,
    tx: &mpsc::UnboundedSender<ServerMsg>,
) -> Result<(String, JoinHandle<()>), ServerMsg> {
    validate_submit(&source, target.as_deref())?;
    // 校验过一次就把它定下来:下面 meta 与 JobSpec 都要用它。漏掉任何
    // 一处,下拉框选了经典也会按超限编译。
    let target = target
        .unwrap_or_else(|| crate::protocol::DEFAULT_TARGET.to_string());

    let bake = state.baker.status();
    let (Some(commit), Some(tag)) = (bake.serving.clone(), bake.serving_tag.clone()) else {
        return Err(err(ErrorCode::NotReady, "镜像还在烘焙,过一会儿再试"));
    };

    let ticket = match state.queue.try_acquire(ip) {
        Ok(t) => t,
        Err(crate::queue::SubmitError::IpBusy) => {
            return Err(err(ErrorCode::IpBusy, ErrorCode::ip_busy_message()));
        }
        Err(crate::queue::SubmitError::QueueFull) => {
            return Err(err(ErrorCode::QueueFull, "排队的人满了,过一会儿再试"));
        }
    };

    // 排队里就先把镜像引用握住:等在前面的时候 gc 可能正要清理旧镜像,
    // 计数是它唯一的依据。
    let guard = state.registry.acquire(&tag);

    if !ticket.is_running() {
        let snap = state.queue.snapshot();
        let _ = tx.send(ServerMsg::Queued {
            position: ticket.position(),
            queue: snap.waiting,
            max_jobs: snap.max_jobs,
        });
    }

    let state = Arc::clone(state);
    let tx = tx.clone();
    let task = crate::job::new_task_id();
    // 任务名和 handle 一起交回去:`cancel` 要把 task 原样回给客户端。
    let spawned = task.clone();
    let handle = tokio::spawn(async move {
        // 额度交接在这里等 —— 不阻塞上面的读循环。
        ticket.wait_started().await;
        let _guard = guard;
        let _ = tx.send(ServerMsg::Started {
            task: task.clone(),
            image_commit: commit.clone(),
        });

        let outcome = run_one(&state, &task, &source, &tag, &commit, &target, &tx).await;
        let _ = tx.send(outcome);
        // _guard 与 ticket 在这里 drop:额度、IP、镜像计数一起还。
    });
    Ok((spawned, handle))
}

async fn run_one(
    state: &Arc<AppState>,
    task: &str,
    source: &str,
    tag: &str,
    commit: &str,
    target: &str,
    tx: &mpsc::UnboundedSender<ServerMsg>,
) -> ServerMsg {
    let paths = match state.jobs.prepare(task, source) {
        Ok(p) => p,
        Err(e) => {
            return err(ErrorCode::Internal, format!("建任务目录失败:{e}"));
        }
    };
    let meta = JobMeta {
        task: task.to_string(),
        target: target.to_string(),
        image_tag: tag.to_string(),
        image_commit: commit.to_string(),
        created_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        source_bytes: source.len(),
    };
    if let Err(e) = state.jobs.write_meta(&paths, &meta) {
        return err(ErrorCode::Internal, format!("写 meta 失败:{e}"));
    }

    let spec = JobSpec {
        task: task.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        image_tag: tag.to_string(),
        image_commit: commit.to_string(),
        paths,
    };
    let sink_tx = tx.clone();
    let sink_task = task.to_string();
    let log: crate::runner::LogSink = Arc::new(move |stream: Stream, text: &str| {
        let _ = sink_tx.send(ServerMsg::Log {
            task: sink_task.clone(),
            stream,
            text: text.to_string(),
        });
    });

    match state.runner.run(&spec, log).await {
        Ok(out) => ServerMsg::Finished {
            task: task.to_string(),
            ok: true,
            bytes: out.artifact_bytes,
            download: format!("/artifact/{task}"),
        },
        Err(e) => describe_error(&e, task),
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::*;
    use crate::{
        bake::{BakeRequest, Baker, BuiltImage},
        config::resolve,
        gc::DiskGate,
        image::ImageRegistry,
        job::{ARTIFACT_NAME, JobStore},
        queue::Queue,
        repo::Repo,
        server::http::{router, AppState},
        testing::{Behavior, FakeBuilder, FakeRunner, OkGate},
    };

    type Ws = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    /// 一个"已经烘好 aaa1111"的后端,`runner` 顶掉真容器。
    fn ready_state(root: &std::path::Path, runner: Arc<FakeRunner>) -> Arc<AppState> {
        ready_state_with(root, runner, |_| {})
    }

    /// `ready_state` 加一个改配置的口子 —— `cfg` 是起服务**之前**定死的,
    /// 服务跑起来之后就动不了了,想测短周期只能在这时候改。
    fn ready_state_with(
        root: &std::path::Path,
        runner: Arc<FakeRunner>,
        tweak: impl FnOnce(&mut crate::config::Config),
    ) -> Arc<AppState> {
        build_state(root, runner, false, tweak)
    }

    /// 同 `ready_state_with`,但装一个 `LspService`。它此刻不碰 docker ——
    /// 只有 `lsp_open` 取会话时才会。
    fn ready_state_with_lsp(root: &std::path::Path, runner: Arc<FakeRunner>) -> Arc<AppState> {
        build_state(root, runner, true, |_| {})
    }

    fn build_state(
        root: &std::path::Path,
        runner: Arc<FakeRunner>,
        with_lsp: bool,
        tweak: impl FnOnce(&mut crate::config::Config),
    ) -> Arc<AppState> {
        let mut cfg = resolve(&|_| None, &root.join("playground")).unwrap();
        cfg.repo_dir = root.to_path_buf();
        cfg.data_dir = root.join("data");
        tweak(&mut cfg);
        let cfg = Arc::new(cfg);
        let baker = Baker::new(
            FakeBuilder::new(vec![Ok(BuiltImage {
                sha: "aaa1111".into(),
                tag: "rust2genshin-play:aaa1111".into(),
            })]),
            Arc::new(OkGate) as Arc<dyn DiskGate>,
        );
        let lsp = with_lsp.then(|| {
            crate::lsp::service::LspService::new(Arc::clone(&cfg), Arc::clone(&baker))
        });
        Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker,
            jobs: Arc::new(JobStore::new(cfg.data_dir.join("tasks"))),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner,
            started: std::time::Instant::now(),
            cfg,
            lsp,
        })
    }

    /// 起一个真服务(127.0.0.1:0),拿一个真 ws 客户端连上去。
    /// 不需要 docker:`FakeRunner` 顶掉容器那一段。
    async fn serve(state: Arc<AppState>) -> (String, Arc<AppState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(Arc::clone(&state));
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        (format!("ws://{addr}/ws"), state)
    }

    /// 建一个临时工作区 + 后端的组合。返回的 `TempDir` 必须活到测试结束,
    /// 别 `mem::forget`(那样每次测试都往 %TEMP% 里漏目录)。
    async fn spawn(runner: Arc<FakeRunner>) -> (tempfile::TempDir, String, Arc<AppState>) {
        spawn_tweaked(runner, |_| {}).await
    }

    /// 同 `spawn`,但能在**起服务之前**改配置。
    async fn spawn_tweaked(
        runner: Arc<FakeRunner>,
        tweak: impl FnOnce(&mut crate::config::Config),
    ) -> (tempfile::TempDir, String, Arc<AppState>) {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("demo/src")).unwrap();
        std::fs::write(d.path().join(SOURCE_FILE), "fn sample() {}").unwrap();
        let state = ready_state_with(d.path(), runner, tweak);
        // 先把 baker 烘好,不然每个用例都得先处理一条 not_ready。
        state.baker.request(BakeRequest::default());
        state.baker.step().await;
        let (url, state) = serve(state).await;
        (d, url, state)
    }

    /// 带 `LspService` 的临时工作区。服务本身不碰 docker;只有 `lsp_open`
    /// 走到取会话那一步才会(本模块的测试都不走到)。
    async fn spawn_with_lsp(
        runner: Arc<FakeRunner>,
    ) -> (tempfile::TempDir, String, Arc<AppState>) {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("demo/src")).unwrap();
        std::fs::write(d.path().join(SOURCE_FILE), "fn sample() {}").unwrap();
        let state = ready_state_with_lsp(d.path(), runner);
        state.baker.request(BakeRequest::default());
        state.baker.step().await;
        let (url, state) = serve(state).await;
        (d, url, state)
    }

    /// 读到第一条满足条件的消息(用于 LSP 的异步推送)。上限比 `read_n`
    /// 长 —— RA 冷启动、诊断都要秒级。
    async fn read_until(ws: &mut Ws, mut pred: impl FnMut(&ServerMsg) -> bool) -> ServerMsg {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let msg = tokio::time::timeout(left, ws.next())
                .await
                .expect("等消息等超时了")
                .expect("连接断了")
                .unwrap();
            if let tokio_tungstenite::tungstenite::Message::Text(t) = msg {
                let m: ServerMsg =
                    serde_json::from_str(&t).expect("服务发的东西必须是 ServerMsg");
                if pred(&m) {
                    return m;
                }
            }
        }
    }

    /// 连上去,收 `n` 条消息(或直到超时)。收到的必须都是 ServerMsg。
    async fn read_n(ws: &mut Ws, n: usize) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        for _ in 0..n {
            let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
                .await
                .expect("消息来晚了")
                .expect("连接断了")
                .unwrap();
            if let tokio_tungstenite::tungstenite::Message::Text(t) = msg {
                out.push(serde_json::from_str(&t).expect("服务发的东西必须是 ServerMsg"));
            }
        }
        out
    }

    async fn connect(url: &str) -> Ws {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        ws
    }

    async fn send(ws: &mut Ws, m: ClientMsg) {
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&m).unwrap().into(),
        ))
        .await
        .unwrap();
    }

    fn submit(src: &str) -> ClientMsg {
        ClientMsg::Submit { source: src.into(), target: None }
    }

    // ── 纯校验:不需要连接,不需要 docker ─────────────────────────────

    #[test]
    fn validate_rejects_empty_and_whitespace_only_sources() {
        // Review Focus #1:空白源码必须在**占额度之前**被挡住,不能让它
        // 起一个注定编不过的容器、占住这个 IP 的排队位。
        for bad in ["", " ", "\n", "\t \r\n"] {
            let m = validate_submit(bad, None).unwrap_err();
            match m {
                ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::BadRequest, "{bad:?}"),
                other => panic!("期望 error,得到 {other:?}"),
            }
        }
    }

    #[test]
    fn validate_rejects_an_oversized_source() {
        // Review Focus #5:上限是"先量再分配"的依据,不能只靠消息上限兜底。
        let big = "x".repeat(MAX_SOURCE_BYTES + 1);
        assert!(validate_submit(&big, None).is_err());
        // 刚好卡在上限上是合法的 —— 边界不能off-by-one。
        let exact = "x".repeat(MAX_SOURCE_BYTES);
        assert!(validate_submit(&exact, None).is_ok());
    }

    #[test]
    fn validate_submit_accepts_both_targets() {
        for t in crate::protocol::TARGETS {
            assert!(
                validate_submit("fn main() {}", Some(t)).is_ok(),
                "{t} 应该在白名单里 —— 它在 protocol::TARGETS 上"
            );
        }
        assert!(validate_submit("fn main() {}", None).is_ok());
    }

    #[test]
    fn validate_submit_names_the_rejected_value_and_the_whitelist() {
        // 拒绝时必须说清「不认识哪个」和「只有哪几个」,否则前端只能把
        // 这句话原样甩给用户。
        let Err(ServerMsg::Error { message, .. }) = validate_submit("fn main() {}", Some("nope"))
        else {
            panic!("nope 应该被拒")
        };
        assert!(message.contains("nope"), "{message}");
        for t in crate::protocol::TARGETS {
            assert!(message.contains(t), "{message} 应该提到 {t}");
        }
    }

    #[test]
    fn validate_lets_a_normal_source_through() {
        assert!(validate_submit("fn main() {}", None).is_ok());
    }

    #[test]
    fn describe_error_says_which_kind_of_failure_it_was() {
        // 三种失败给用户看的话必须不一样 —— 尤其是超时:它的容器既没有
        // 退出码也没有产物,落到"NoArtifact"那条上就成了"编译没产出",
        // 用户会去改代码,而问题是他写了个死循环。
        let t = describe_error(
            &crate::runner::RunnerError::Timeout(Duration::from_secs(15)),
            "t1",
        );
        match t {
            ServerMsg::Failed { message, exit_code, .. } => {
                assert!(message.contains("15"), "要说清超了多少秒:{message}");
                assert_eq!(exit_code, None);
            }
            other => panic!("期望 failed,得到 {other:?}"),
        }

        let n = describe_error(
            &crate::runner::RunnerError::NoArtifact { code: Some(101) },
            "t1",
        );
        match n {
            ServerMsg::Failed { exit_code, .. } => assert_eq!(exit_code, Some(101)),
            other => panic!("期望 failed,得到 {other:?}"),
        }
    }

    // ── 整条链路 ────────────────────────────────────────────────────

    #[tokio::test]
    async fn hello_carries_the_serving_commit() {
        let (_d, url, _st) = spawn(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Hello { protocol, image_commit, file, max_jobs, targets, .. } => {
                assert_eq!(protocol, PROTOCOL_VERSION);
                assert_eq!(image_commit.as_deref(), Some("aaa1111"));
                assert_eq!(file, SOURCE_FILE);
                assert_eq!(max_jobs, 8);
                assert_eq!(
                    targets,
                    vec!["beyond-server".to_string(), "classic-server".to_string()]
                );
            }
            other => panic!("第一条必须是 hello,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn hello_lists_both_targets() {
        // Task 10 的下拉框由这张表驱动:少一条,前端就选不到经典模式。
        let (_d, url, _st) = spawn(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Hello { targets, .. } => assert_eq!(
                targets,
                vec!["beyond-server".to_string(), "classic-server".to_string()]
            ),
            other => panic!("期望 hello,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn lsp_open_is_rejected_when_lsp_is_disabled() {
        let (_d, url, _st) = spawn(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Hello { lsp_supported, .. } => assert!(!lsp_supported),
            other => panic!("期望 hello,得到 {other:?}"),
        }
        send(&mut ws, ClientMsg::LspOpen {}).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { message, .. } => assert!(message.contains("分析"), "{message}"),
            other => panic!("期望 error,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn hello_reports_lsp_supported_when_the_service_is_configured() {
        // `AppState::lsp` 有值 = 前端可以挂 LSP 客户端。这条钉住 hello 不再
        // 写死 `false`。
        let (_d, url, _st) =
            spawn_with_lsp(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Hello { lsp_supported, .. } => assert!(lsp_supported),
            other => panic!("期望 hello,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn lsp_change_without_open_is_a_bad_request() {
        // 没开会话就发源码:要明确回「先发 lsp_open」,不能静默吞掉 ——
        // 前端会一直以为自己在等分析结果。
        let (_d, url, _st) =
            spawn_with_lsp(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, ClientMsg::LspChange { source: "fn main() {}".into() }).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, message } => {
                assert_eq!(code, ErrorCode::BadRequest);
                assert!(message.contains("lsp_open"), "{message}");
            }
            other => panic!("期望 error,得到 {other:?}"),
        }
    }

    /// 真起一个 RA 会话,走完整条链路,再验证断开后会话回池、能被下一条
    /// 连接复用。需要本机有 docker 与烘好的镜像。
    #[tokio::test]
    #[ignore = "需要本机有 docker 与烘好的镜像"]
    async fn lsp_session_serves_diagnostics_and_is_reused_after_disconnect() {
        // 本机没有当前 HEAD 的镜像就跳过 —— 起不了 RA。
        let mut cfg = crate::config::load().expect("读配置");
        let repo = Repo::new(cfg.repo_dir.clone());
        let Some(built) = crate::startup_image(&cfg, &repo).await else {
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

        // 先直接取一次再放回,记下容器名。待命会话在池里只有这一个,
        // 所以后面复用的一定是它;若池没把会话放回去,新建的会拿到
        // 下一个序号,名字就对不上了。
        let first = svc.acquire().await.expect("镜像就绪该能起 RA");
        let container = first.container().to_string();
        assert!(!container.is_empty(), "真实会话要带容器名");
        first.release(true).await;

        let (url, _state) = serve(state).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, ClientMsg::LspOpen {}).await;
        let ready = read_until(&mut ws, |m| matches!(m, ServerMsg::LspReady { .. })).await;
        match ready {
            ServerMsg::LspReady { file } => assert_eq!(file, SOURCE_FILE),
            other => panic!("期望 lsp_ready,得到 {other:?}"),
        }

        // 有语法错 → RA 必然报一条;空 vec 是「清空」,不算。
        send(
            &mut ws,
            ClientMsg::LspChange {
                source: "fn main() { let x: i32 = \"not an int\"; }".into(),
            },
        )
        .await;
        read_until(&mut ws, |m| {
            matches!(m, ServerMsg::LspDiagnostic { message, .. } if !message.is_empty())
        })
        .await;

        // 断开 → 会话应回池。给它一点时间走完清理(转发任务 abort +
        // `release(alive)`),再取一次应是同一容器。
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

    #[tokio::test]
    async fn a_submit_runs_all_the_way_to_finished() {
        let runner = FakeRunner::new(Behavior::Ok {
            lines: vec![(Stream::Stdout, "Compiling\n".into())],
            bytes: 15860,
        });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, submit("fn main() {}")).await;
        // FakeRunner 只吐一行日志,所以正好三条:started / log / finished。
        let msgs = read_n(&mut ws, 3).await;

        assert!(matches!(msgs[0], ServerMsg::Started { .. }), "{:?}", msgs[0]);
        match &msgs[1] {
            ServerMsg::Log { text, stream, .. } => {
                assert_eq!(text, "Compiling\n");
                assert_eq!(*stream, Stream::Stdout);
            }
            other => panic!("期望 log,得到 {other:?}"),
        }
        let task = match &msgs[2] {
            ServerMsg::Finished { ok, bytes, download, task } => {
                assert!(*ok);
                assert_eq!(*bytes, 15860);
                assert_eq!(download, &format!("/artifact/{task}"));
                task.clone()
            }
            other => panic!("期望 finished,得到 {other:?}"),
        };
        assert_eq!(runner.calls(), vec![task.clone()], "任务 id 要和 runner 收到的一致");
        assert!(st.jobs.artifact_of(&task).is_some(), "产物要真落在盘上");
        assert_eq!(st.queue.snapshot().running, 0, "跑完要把额度还回去");
    }

    #[tokio::test]
    async fn submit_with_classic_target_reaches_the_runner() {
        // 光放行 `classic-server` 不够:落盘的 meta 和交给 Runner 的 JobSpec
        // 也得带上它,否则下拉框选了经典仍按超限编译,磁盘上还写着错的。
        let runner = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 42 });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(
            &mut ws,
            ClientMsg::Submit {
                source: "fn main(){}".into(),
                target: Some("classic-server".into()),
            },
        )
        .await;
        let msgs = read_n(&mut ws, 2).await; // started / finished
        let task = match &msgs[1] {
            ServerMsg::Finished { task, .. } => task.clone(),
            other => panic!("期望 finished,得到 {other:?}"),
        };
        let meta: JobMeta = st.jobs.read_meta(&task).unwrap();
        assert_eq!(meta.target, "classic-server");
        assert_eq!(
            runner.targets(),
            vec!["classic-server".to_string()],
            "Runner 收到的 JobSpec.target 必须是选中的那个"
        );
    }

    /// 任务跑完之后连接得**还开着**。
    ///
    /// 读循环和任务之间靠一条 mpsc 通道联系着:`run_socket` 建 `(tx, rx)`,
    /// 把 `tx.clone()` 交给任务。任务结束、手里那份 drop 之后,如果没有任何
    /// 东西还握着 `tx`,`rx.recv()` 就返回 `None`,读循环当成"没人要再发消息
    /// 了"直接 break —— socket 被后端自己关掉。表现是浏览器那边 `finished`
    /// 连同 .gia 下载链接一起收不到,页面永远停在「编译中…」。
    ///
    /// 所以这里在 finished 之后再发一次 submit:回不回得来是次要的,关键是
    /// 这条连接还能不能把消息送出去 —— 能,说明读循环还活着。
    #[tokio::test]
    async fn a_finished_job_leaves_the_connection_open() {
        let runner = FakeRunner::new(Behavior::Ok {
            lines: vec![(Stream::Stdout, "Compiling\n".into())],
            bytes: 15860,
        });
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, submit("fn main() {}")).await;
        let msgs = read_n(&mut ws, 3).await; // started / log / finished
        assert!(matches!(msgs[2], ServerMsg::Finished { .. }), "{:?}", msgs[2]);

        send(&mut ws, submit("fn main() {}")).await;
        let back = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("5 秒内该有回音 —— 连接要是被后端关了,这里就超时")
            .expect("连接断了")
            .unwrap();
        let back: ServerMsg =
            serde_json::from_str(back.to_text().expect("回音应当是文本")).expect("回音应当是 JSON");
        // **不钉具体是哪一条**。这里要证的只有「连接还活着」,而第一条任务
        // 收尾时就把额度还了,所以第二次提交拿到 `Started` 是正常的;偶尔
        // 抢在 drop 尾巴上会拿到 `IpBusy`。钉死成 `Error` 会让这条测试
        // 在机器调度快一点时就红 —— 它测的就成了「额度什么时候还」,
        // 那是别的用例的事。
        assert!(
            matches!(
                back,
                ServerMsg::Started { .. } | ServerMsg::Queued { .. } | ServerMsg::Error { .. }
            ),
            "得到 {back:?}"
        );
    }

    #[tokio::test]
    async fn a_second_submit_on_the_same_connection_is_rejected() {
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        read_n(&mut ws, 1).await; // started
        send(&mut ws, submit("fn main() {}")).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::BadRequest),
            other => panic!("期望 bad_request,得到 {other:?}"),
        }
        assert_eq!(runner.calls().len(), 1);
    }

    #[tokio::test]
    async fn same_ip_on_a_second_connection_is_rejected() {
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut a = connect(&url).await;
        read_n(&mut a, 1).await;
        send(&mut a, submit("fn main() {}")).await;
        read_n(&mut a, 1).await; // started

        let mut b = connect(&url).await;
        read_n(&mut b, 1).await; // hello
        send(&mut b, submit("fn main() {}")).await;
        match read_n(&mut b, 1).await[0].clone() {
            ServerMsg::Error { code, message } => {
                assert_eq!(code, ErrorCode::IpBusy);
                assert_eq!(message, "请求过多，请稍后重试");
            }
            other => panic!("期望 ip_busy,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_empty_source_never_reaches_the_runner() {
        let runner = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("   \n\t ")).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::BadRequest),
            other => panic!("期望 bad_request,得到 {other:?}"),
        }
        assert!(runner.calls().is_empty(), "空源码不该起容器");
        assert_eq!(st.queue.snapshot().running, 0);
        assert_eq!(st.queue.snapshot().waiting, 0);
    }

    /// 空闲连接要自己出声。前面 nginx stream 层按「最后一次有字节」掐空闲,
    /// 排队等镜像的那几十秒里一条业务消息都没有 —— 不 ping 的话连接会被悄悄
    /// 关掉,`finished` 连同 .gia 下载链接一起丢在半路(2026-10-17 实测)。
    #[tokio::test]
    async fn an_idle_connection_gets_pings() {
        let (_d, url, _state) = spawn_tweaked(FakeRunner::new(Behavior::Hang), |cfg| {
            // 周期调到 100ms,不用真等默认的 25s。
            cfg.keepalive = Duration::from_millis(100);
        })
        .await;
        let mut ws = connect(&url).await;
        // 先吃掉 hello,再等 ping。
        assert_eq!(read_n(&mut ws, 1).await.len(), 1);
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("5 秒内该有 ping")
            .expect("连接断了")
            .unwrap();
        assert!(
            matches!(msg, tokio_tungstenite::tungstenite::Message::Ping(_)),
            "空闲时收到的是 {msg:?},不是 Ping"
        );
    }

    #[tokio::test]
    async fn a_failed_run_reports_failed_with_the_exit_code() {
        let runner = FakeRunner::new(Behavior::Fail {
            lines: vec![(Stream::Stderr, "error[E0308]\n".into())],
            code: 101,
        });
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        let msgs = read_n(&mut ws, 3).await; // started / log / failed
        match &msgs[2] {
            ServerMsg::Failed { exit_code, .. } => assert_eq!(*exit_code, Some(101)),
            other => panic!("期望 failed,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn not_ready_is_reported_before_the_first_bake() {
        // 新的 baker 一次都没烘成功过 —— 提交必须回 not_ready,
        // 而不是排进队列然后等一个不存在的镜像。
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("demo/src")).unwrap();
        let st = ready_state(
            d.path(),
            FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 }),
        );
        let (url, _st) = serve(st).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::NotReady),
            other => panic!("期望 not_ready,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn disconnecting_mid_job_frees_the_quota_and_the_ip() {
        // Review Focus #3:连接断在半路必须把额度还回去,否则一个客户端
        // 跑到一半挂断,能把这个 IP 和并发位永远占住。
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        read_n(&mut ws, 1).await; // started
        assert_eq!(st.queue.snapshot().running, 1);

        drop(ws);
        for _ in 0..100 {
            if st.queue.snapshot().running == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(st.queue.snapshot().running, 0, "断开后额度必须还回去");

        // 而且同一个 IP 立刻能再提交。
        let mut again = connect(&url).await;
        read_n(&mut again, 1).await;
        send(&mut again, submit("fn main() {}")).await;
        assert!(
            matches!(read_n(&mut again, 1).await[0], ServerMsg::Started { .. }),
            "释放之后同一个 IP 应该能重新提交"
        );
    }

    #[tokio::test]
    async fn cancel_releases_the_slot() {
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        read_n(&mut ws, 1).await; // started
        send(&mut ws, ClientMsg::Cancel {}).await;
        for _ in 0..100 {
            if st.queue.snapshot().running == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(st.queue.snapshot().running, 0);
    }

    #[tokio::test]
    async fn cancel_answers_the_client_with_cancelled() {
        // 点「取消」之后前端只会停在「编译中…」—— 它在等服务端一条终结
        // 消息,而 `abort` 是**静默**的,不产生任何消息。这条钉住「取消
        // 要有回音」;回音还必须带 task,前端靠它认这是不是自己那一单。
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, submit("fn main() {}")).await;
        let started = read_n(&mut ws, 1).await; // started
        let task = match &started[0] {
            ServerMsg::Started { task, .. } => task.clone(),
            other => panic!("期望 started,得到 {other:?}"),
        };
        send(&mut ws, ClientMsg::Cancel {}).await;

        // 不用 `read_n`:它要求消息能反序列化成 ServerMsg,而这条测试的
        // 意义正是在 ServerMsg 还没有 `cancelled` 时先红。
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("取消之后必须回一条终结消息,否则前端一直停在「编译中…」")
            .expect("连接断了")
            .unwrap();
        let text = match msg {
            tokio_tungstenite::tungstenite::Message::Text(t) => t,
            other => panic!("期望文本消息,得到 {other:?}"),
        };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["type"], "cancelled");
        assert_eq!(v["task"], task);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_internal_error_also_frees_the_connection_slot() {
        // `run_one` 在建任务目录 / 写 meta 失败时返回 `Error{Internal}` ——
        // 那个任务其实已经结束、`ticket`/`guard` 也还了,**但消息不是
        // `Finished`/`Failed`**。连接若因此把槽位留成 `Some`,下一次提交
        // 会被误判成"已有任务在跑",与真实状态不符。
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("demo/src")).unwrap();
        // tasks 根指向一个**普通文件**:`prepare` 里的 `create_dir_all` 必然失败。
        let tasks_file = d.path().join("tasks");
        std::fs::write(&tasks_file, b"x").unwrap();

        let mut cfg = resolve(&|_| None, &d.path().join("playground")).unwrap();
        cfg.repo_dir = d.path().to_path_buf();
        cfg.data_dir = d.path().join("data");
        let cfg = Arc::new(cfg);
        let st = Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker: Baker::new(
                FakeBuilder::new(vec![Ok(BuiltImage {
                    sha: "aaa1111".into(),
                    tag: "rust2genshin-play:aaa1111".into(),
                })]),
                Arc::new(OkGate) as Arc<dyn DiskGate>,
            ),
            jobs: Arc::new(JobStore::new(tasks_file)),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner: FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 }),
            started: std::time::Instant::now(),
            cfg,
            lsp: None,
        });
        // baker 必须 ready,否则先回 NotReady、测不到 Internal 这条路径。
        st.baker.request(BakeRequest::default());
        st.baker.step().await;
        let (url, _st) = serve(st).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello

        send(&mut ws, submit("fn main() {}")).await;
        // Started 先发,然后才是 run_one 的 Internal。
        let msgs = read_n(&mut ws, 2).await;
        assert!(matches!(msgs[0], ServerMsg::Started { .. }), "{:?}", msgs[0]);
        match msgs[1].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::Internal),
            other => panic!("期望 internal,得到 {other:?}"),
        }

        // 那个任务已经结束了,槽位必须空了:再来一次不该被当成"已有任务在跑"。
        send(&mut ws, submit("fn main() {}")).await;
        let first = read_n(&mut ws, 1).await[0].clone();
        assert!(
            matches!(first, ServerMsg::Started { .. }),
            "槽位没清空,第二次提交被误判成:{first:?}"
        );
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::Internal),
            other => panic!("期望 internal,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_browser_origin_that_is_not_allowed_gets_a_403() {
        // WebSocket 不吃 CORS —— 这一条要是没有,任何网页都能拿访客的
        // 浏览器当跳板打这个后端。
        let (_d, url, _st) = spawn(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let req = tokio_tungstenite::tungstenite::handshake::client::Request::builder()
            .uri(&url)
            .header("Origin", "https://evil.example")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .header("Host", "127.0.0.1")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .body(())
            .unwrap();
        let err = tokio_tungstenite::connect_async(req).await.unwrap_err();
        assert!(
            format!("{err}").contains("403"),
            "没在白名单里的 Origin 必须被握手挡掉:{err}"
        );
    }

    // 「不带 Origin 的客户端放行」不单写一条:上面每条 `connect` 走的都是
    // 这条路(tokio-tungstenite 默认不发 Origin),它们能连通本身就是这个
    // 保底的另一半。

    #[tokio::test]
    async fn the_task_meta_records_which_image_ran_it() {
        let runner = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 42 });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        // 这个 runner 不吐日志,所以只有两条:started / finished。
        let msgs = read_n(&mut ws, 2).await;
        let task = match &msgs[1] {
            ServerMsg::Finished { task, .. } => task.clone(),
            other => panic!("期望 finished,得到 {other:?}"),
        };
        let meta: JobMeta = st.jobs.read_meta(&task).unwrap();
        assert_eq!(meta.task, task);
        assert_eq!(meta.image_tag, "rust2genshin-play:aaa1111");
        assert_eq!(meta.image_commit, "aaa1111");
        assert_eq!(meta.target, "beyond-server");
        assert_eq!(meta.source_bytes, "fn main() {}".len());
        assert_eq!(
            st.jobs.artifact_of(&task).unwrap().file_name().unwrap(),
            ARTIFACT_NAME
        );
    }
}
