# 摆弄页：会话级 IP 限制 + 代码分析大小上限（设计稿）

日期：2026-10-21

## 背景

现在的「每 IP 一个」限制**只管编译**：

- `queue.rs` 的 `try_acquire(ip)` 只在 `server/ws.rs` 的 `start()`（提交编译）里被调用，票在**编译任务结束**时随 `Ticket` 的 `Drop` 归还。
- 代码分析（rust-analyzer，RA）这条路**完全不经过队列**：`lsp_open` 直接从 `lsp/pool.rs` 的会话池取会话，池空就现造一个容器（`lsp/service.rs` 的 `SessionMaker::make`：`docker run -d` + `docker exec -i rust-analyzer`）。

后果：同一个 IP 可以开任意条 WebSocket 连接，每条各拿一个 RA 会话 —— 每个 RA 进程做完一次 `didOpen` 常驻 839 MB，而会话池**只有空闲下限 `lsp_pool_min` 和空闲回收，没有在用会话数上限**。于是「每 IP 一个」这条限流对真正贵的资源（RA 容器与内存）形同虚设。

同时，RA 对源码体量没有上限：源码越大，RA 的 crate graph 越重，内存与 CPU 一起涨。应当在超过阈值时干脆**不调 RA**，并明确告诉用户。

## 目标

1. 「每 IP 一个」的粒度从**编译任务**提升到**连接（会话）**：一个 IP 同时只能有一条活连接，因而也只能有一个 RA 会话、一个在途编译。
2. 源码超过阈值（默认 10 KiB）时**不调 RA**，并给出提示；**服务端也要校验**（前端只是快路径，服务端才是闸门）。

## 不做

- 不做全局 RA 会话数上限（跨 IP 的滥用防护）。会话级限制已经掐住「一个人开一堆标签页」这一主要场景；全局上限是另一件事，本次不引入。
- 不动编译路径的 1 MiB 上限（`protocol::MAX_SOURCE_BYTES`）与并发/排队上限（`max_jobs` / `max_queue`）。

## 设计

### 一、会话级 IP 限制

**新增 `playground/src/server/session_gate.rs`**：

```rust
pub struct SessionGate { inner: Mutex<HashSet<String>> }
impl SessionGate {
    pub fn acquire(self: &Arc<Self>, ip: &str) -> Result<SessionGuard, SessionBusy>;
}
pub struct SessionGuard { gate: Arc<SessionGate>, ip: String }  // Drop 里移除
```

- 占用在 `run_socket` 的**最开头**取，`SessionGuard` 随连接结束（无论是正常断开、协议错、任务收尾）drop，自动归还。没有手工 `release()` 的路径。
- 取不到时：服务端发一条 `ServerMsg::Error { code: SessionBusy }`，然后**关闭连接**。不发 `hello` —— 这个连接根本不该被当成可用会话。
- 复用现成的 `server::client_key_of`（IPv6 按 /64 归并、IPv4-mapped 归一），所以「换地址绕过」这条路仍然堵着。
- **握手阶段不拒绝**。在 `ws_upgrade` 里 403 掉的话，前端的自动重连会变成「403 → 重连 → 403」的死循环；改成「握手成功 → 收到一条明确错误 → 前端停止自动重连」，用户能看懂发生了什么。

**协议**：给 `ErrorCode` 加一个 `SessionBusy`（线上是 `"session_busy"`），和编译路径的 `IpBusy`（`"ip_busy"`）区分开 —— 前端要靠它判断「这是会话被占，别再重连」。

**队列里的 IP 维度保留**。连接级限制已经保证一 IP 一连接，`queue` 的 `ips` 集合因此不再会被触发；留着它是第二道防线（万一会话闸门出 bug，编译路径仍会拒）。两处判重不冲突：会话闸门拒的是**第二条连接**，队列拒的是**第二个编译任务**。

**前端**：收到 `session_busy` 错误后

- 状态行显示「同一个 IP 已经有另一个会话在用（另一个标签页？）—— 本页不再自动重连」；
- `PlayClient` 置一个 `_sessionRejected` 标志，`ensureConnected()` / `connect()` 见到它就直接返回 false，不再自动重连（用户刷新页面才重试）。

### 二、代码分析大小上限

**配置**：`PLAYGROUND_LSP_MAX_SOURCE_BYTES`，默认 `10240`（10 KiB）。**按 UTF-8 字节算**，不是字符数 —— 中文一个字 3 字节。`protocol.rs` 里已有同样的坑（`MAX_SOURCE_BYTES` 的注释）。

**协议**：`hello` 加一个字段 `lsp_max_source_bytes`。前端不在代码里写死 10 KB —— 阈值由服务端说了算，改配置就够。

**前端（快路径）**：`LspClient` 持有 `maxBytes`。

- `open()` 之前先量当前文档的字节数：超了就不发 `lsp_open`（**连 RA 容器都不起**），状态行提示「源码超过 10 KB，已停用代码分析」。
- 编辑过程中跨过阈值：停发 `didChange`，同样提示；改回阈值以下自动恢复（没开过就补一次 `open()`，开过就补一次 `didChange`）。
- 字节数用 `TextEncoder().encode(text).length` 算 —— `String.length` 是 UTF-16 code unit 数，与服务端的字节数对不上。

**服务端（闸门）**：`server/ws.rs` 的 `LspRoute::Store`（`didOpen` / `didChange`）分支，拿到全文后：

- `text.len() > cfg.lsp_max_source_bytes` → **不写盘、不转发给 RA**，回一条 `Error { code: BadRequest }`，消息说清「源码 N 字节，超过 M 字节上限，代码分析已停用」；
- 编译路径不受影响（1 MiB 上限照旧）。

**为什么两条都要**：前端那道是快路径（省一次往返、省一次容器冷启动），服务端那道是闸门（前端可以被改坏、可以被绕过；阈值是运维配置，不能只靠浏览器自觉）。

## 改动清单

后端：

- `playground/src/protocol.rs`：`Hello` 加 `lsp_max_source_bytes`；`ErrorCode` 加 `SessionBusy`；`PROTOCOL_VERSION` 3 → 4。
- `playground/src/config.rs`：加 `lsp_max_source_bytes`（默认 10240）。
- `playground/src/server/session_gate.rs`（新）：`SessionGate` + `SessionGuard`。
- `playground/src/server/http.rs`：`AppState` 加 `sessions: Arc<SessionGate>`。
- `playground/src/server/ws.rs`：连接开头取会话占用；`Store` 路由加大小校验。
- `playground/src/main.rs`：装配 `SessionGate`。

前端：

- `docs/typst/play/lsp-client.js`：`maxBytes` 与超限停发/恢复。
- `docs/typst/play/script.js`：把 `hello.lsp_max_source_bytes` 传给 `LspClient`；处理 `session_busy`（提示 + 停止自动重连）。

站点：

- `docs/typst/dev/playground.typ`：配置表加 `PLAYGROUND_LSP_MAX_SOURCE_BYTES`；补一段「每 IP 一个会话」的说明。

## 测试

- `session_gate.rs`：第二个同 IP 的占用被拒；guard drop 后能再取；不同 IP 互不影响。
- `server/ws.rs`：同 IP 的第二条连接收到 `session_busy`；断开第一条之后同 IP 能再连；超限的 `didOpen` 被拒且没落盘、没转发。
