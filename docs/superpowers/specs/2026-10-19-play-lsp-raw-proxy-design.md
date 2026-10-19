# 摆弄页 LSP 改为原样转发设计

日期：2026-10-19
状态：待实现

## 目标

把摆弄页的 LSP 通道从「服务端当 LSP 客户端、翻译成专用消息」改成：

- **服务端只处理会话**：管道、待命池、`initialize` 应答、`didOpen` / `didChange` 的落盘与会话边界；
- **其余 LSP 消息一律原样转发**：浏览器发什么就写进 RA 的 stdin，RA 回什么就发给浏览器；
- **前端自己消费**：semantic tokens、拉取式诊断、补全结果都在浏览器里解析；不认识的 LSP 消息原样收下、忽略。

这样以后加 definition、hover、references、rename 不需要改协议。

## 非目标

- 不引入 `@codemirror/lsp-client`。它不支持 semantic tokens，诊断也只认推送式（`publishDiagnostics`），与本页的两项核心能力不匹配；本设计先把传输层做对，导航类能力以后可以直接加在同一个传输上。
- 不做多文件编辑。文档仍然只有一个：`file:///opt/ra/ws/demo/src/lib.rs`（`playground/src/lsp/session.rs:160`）。
- 不改编译任务那条路（`runner` / `FrameDecoder` / 产物二进制帧）。
- 不做 semantic tokens 的 delta 增量，仍按全文重取。

## 现状与问题

今天服务端实现了半个 LSP 客户端：`playground/src/lsp/session.rs` 里做 legend 重映射（`:87-164`）、拉 token（`:553`）、拉诊断并缓存 `unchanged`（`:586`、`:613-767`）、解析补全（`:829`）、按 `request_id` 配对；`playground/src/server/ws.rs:189-320` 再把事件翻成 `LspTokens` / `LspDiagnostics` / `LspCompletion`；前端 `docs/typst/play/lsp-client.js` 有 431 行专门处理这几种消息。

代价不是行数本身，而是**扩展模型**：每加一个 LSP 能力（definition、hover、references、rename）都要同时改四处 —— `protocol.rs` 加消息、`session.rs` 加请求与解析、`ws.rs` 加分发、前端加调用。而这四种能力本来都是标准的 request/response。

`protocol.rs:3-5` 的文件头本来就预留了 `{"type":"lsp", ...}` 双向转发（写于 2026-10-16 的 playground 设计），10-17 的补全设计没有按这条预留走。本设计把它改回来。

## 架构

```text
浏览器
  │  {"type":"lsp","message":{jsonrpc,...}}   ← 除握手外一律原样
  ▼
ws.rs：拆信封 → 少量拦截（见下）→ 写 RA 管道
  │
  ▼
docker exec -i rust-analyzer（待命池里的长驻进程）
  │  stdout 拆 Content-Length 帧
  ▼
ws.rs：包回信封（{"type":"lsp","message":...}）→ 前端
```

服务端仍然拥有三类东西，这些是浏览器拿不到的：

1. `docker exec` 管道（`lsp/docker_exec.rs`、`lsp/exec.rs`）；
2. 待命池与存活检查（`lsp/pool.rs`、`lsp/service.rs`）；
3. 会话建立时的 `initialize`：RA 一个进程只能 initialize 一次，而池里的进程是跨会话复用的。

## 协议

`PROTOCOL_VERSION` 从 2 抬到 3（`playground/src/protocol.rs:11`）。

`ClientMsg` / `ServerMsg` 各加一个变体，LSP 内容放进 `message`，不再定义任何 LSP 字段：

```rust
Lsp { message: serde_json::Value },
```

序列化结果：

```json
{"type":"lsp","message":{"jsonrpc":"2.0","id":7,"method":"textDocument/completion","params":{...}}}
```

`message` 在语义上就是 LSP 报文本身，服务端不重命名字段、不重组结构。服务端会解析出 `method` / `id` 仅用于握手与落盘拦截（见下），其余字段一律原样写回。

`ClientMsg` 保留：

- `LspOpen {}`：仍然表示「取一个 RA 会话」的应用动作，不是 LSP 消息；
- `Lsp { message }`。

删除 `LspChange`、`LspCompletion`。

`ServerMsg` 保留：

- `LspReady { file, uri }`：会话已取得。`uri` 是新加的，前端需要它填 `textDocument.uri`；
- `Lsp { message }`。

删除 `LspTokens`、`LspDiagnostics`、`LspCompletion` 及其结构体。

**关闭信号复用 LSP 的 `exit` 通知**，不新增应用层变体：RA 管道断开或会话被回收时，服务端合成一条 `{"jsonrpc":"2.0","method":"exit","params":{"reason":"..."}}` 放进 `Lsp` 发下去，前端收到即视为传输终止，清高亮/诊断并 reject 全部 pending。

为什么是 `exit` 而不是 `shutdown`：`shutdown` 是带 `id` 的**请求**，语义上要求对端回响应；前端对服务端请求的通用处理是回 `null`，把它当关闭信号会多一次无意义的往返，而且「关了没有」取决于谁先到。`exit` 是**通知**，天然单向，正好当终止信号。

这是本设计里唯一一条**反向**合成的 LSP 帧：`exit` 在规范里是客户端→服务端。浏览器发来的 `exit` 仍然被服务端吞掉，不会回环；前端也不装通用 LSP 客户端，所以不存在语义冲突。用 `params.reason` 携带给用户看的原因（规范未定义该参数，属于本地扩展）。

## 服务端拦截清单

服务端只对以下消息特殊处理，其余（包括所有请求、通知、响应）原样双向转发：

| 方向 | 消息 | 处理 |
|---|---|---|
| 浏览器 → RA | `initialize` | **不转发**。用待命时缓存的真实响应（含 capabilities 与 semanticTokens legend）按浏览器的 `id` 回复 |
| 浏览器 → RA | `initialized` | **不转发**。RA 在待命阶段已经收过一次 |
| 浏览器 → RA | `shutdown` / `exit` | **不转发**。会杀掉池里的共享进程 |
| 浏览器 → RA | `textDocument/didOpen` | 把 `params.textDocument.text` 落盘到容器内 `demo/src/lib.rs`，再原样转发 |
| 浏览器 → RA | `textDocument/didChange` | 当前是全文替换，取 `contentChanges[0].text` 落盘，再原样转发 |
| RA → 浏览器 | 一切 | 原样转发 |
| 服务端 → 浏览器 | `exit`（合成） | 管道断开/会话回收时发一次，`params.reason` 是给用户看的原因；前端视为传输终止 |

落盘仍保留：RA 对 workspace 成员的解析会读磁盘，overlay 只覆盖打开的文件，二者内容要对齐。

### 会话边界

待命阶段（池里预热）仍然由服务端走 `initialize` + 一次 `didOpen(占位)`，目的是把 crate graph 拉热（见 `playground/src/lsp/session.rs:306`）。

**就绪判据要换**：今天 `service.rs:212-227` 靠 `Event::Diagnostics` / `Event::Tokens` 判定「crate graph 已加载」，而这两个事件随着翻译层一起删。改成：`didOpen(占位)` 之后，服务端用**自己的 id** 发一条 `textDocument/diagnostic`，等它的响应回来即视为就绪。这里仍然需要 `Pending` 表做请求配对，但只等「有响应」，不解析诊断内容。请求在交棒前一定会被 `await` 完，交棒后 `Pending` 必为空。

**交棒前**（`acquire` 成功后、发 `LspReady` 之前）服务端补一条 `textDocument/didClose`，让池里的会话变成「没有打开文档」的状态。归还池时若浏览器没有发过 `didClose`，也由服务端补一条。这样：

- 浏览器发来的 `didOpen` 永远合法，可以原样转发；
- 不需要把 `didOpen` 改写成 `didChange`；
- RA 丢掉的是一个文件的 overlay，crate graph 仍然保留。

代价是下一个会话首次 `didOpen` 时 RA 要重新分析这一个文件（毫秒级），可接受。

### id 空间

交棒后服务端不再发任何请求（存活检查 `ping` 挪到交棒之前，`session.rs:476`），所以浏览器的 JSON-RPC `id` 可以原样进出，不需要配对表。RA → 浏览器的请求（如 `client/registerCapability`、`workspace/configuration`）也原样转发；前端对未知请求统一回 `null`，避免 RA 等待。

## 前端

### 传输

`lsp-client.js` 换成一个最小的 JSON-RPC 层：

- `request(method, params) -> Promise`：分配 `id`，登记 pending，超时/收到 `exit` 时 reject；
- `notify(method, params)`；
- `handleMessage(msg)`：有 `method` 且无 `id` 是通知（`exit` 视为传输终止），有 `method` 且有 `id` 是 RA 的请求（回 `null`），其余按 `id` 兑现 Promise；
- 发送时统一包成 `{ type: "lsp", message }`。

握手顺序（收到 `lsp_ready` 后）：`initialize` → `initialized` → `didOpen` → 拉 tokens 与诊断。

`initialize` 的 capabilities 由前端给，但**服务端不会转发它**，实际 capability 以服务端待命时上报的那份为准；前端只消费响应里的 `capabilities.semanticTokensProvider.legend`。

### semantic tokens

- `didOpen` / 每次 `didChange` 落定后发 `textDocument/semanticTokens/full`；
- 收到 `workspace/semanticTokens/refresh` 通知时重发；
- 用 initialize 响应里的真实 legend 把 RA 的索引翻成名字，再映射到 CSS class。名字不在支持列表里的 token 跳过。这份映射从服务端搬到前端，**不再重排索引**；
- delta 数组的解码与 decoration 生成沿用现有 `decodeTokens` / `decorationField` 的结构，只是数据来源换成自己的请求。

### 诊断

- `didOpen` / `didChange` 后发 `textDocument/diagnostic`；
- 结果 `kind == "full"` → 整份替换；`kind == "unchanged"` → 保留上一份；
- 收到 `workspace/diagnostic/refresh` → 重新拉一次；
- 继续兼容推送式 `textDocument/publishDiagnostics`（按同一个 uri 整份替换）。

### 补全

- 去抖 300 ms、2 秒兜底、在途替换沿用现有逻辑（`lsp-client.js:364`）；
- 直接发 `textDocument/completion`，位置按 LSP 约定：0 基行、UTF-16 列；
- 响应解析和现有的 `stripPrefixNoise` / label-detail 清洗不变（`script.js` 的渲染部分）；
- `insertText` 仍然取 `label`，RA 默认一致。

### 重连

页面空闲断开后重连会重新走 `hello` → `lsp_open` → 新会话。前端清空 pending 请求与版本号，重新 `initialize` / `didOpen`；已有 decoration 可以保留到新结果到达，不再需要「`lsp_ready` 之前丢弃 token」的规则 —— 新会话的 token 一定是针对当前文档的。

## 删除清单

| 文件 | 删除内容 |
|---|---|
| `playground/src/lsp/session.rs` | `TokenLegend` 及其重映射、`request_tokens`、`request_diagnostics`、诊断缓存 `last_diagnostics`、`dispatch` 的翻译分支、`emit_diagnostics`、`parse_diagnostic_result`、`diagnostic_from`、`parse_completion_items`；`Event` 换成单个 `Message(Value)`（转发用）。`reader_loop` 保留「拆帧 → 若命中 `Pending` 则配对，否则转发」。`Session` 剩下：进程句柄、`initialize`、`did_open` / `did_close`（待命与边界用）、就绪探测、`ping`、`send_json` |
| `playground/src/server/ws.rs` | `LspConn.forwarding` 与事件翻译任务；LSP 分支改成两个泵（WS→管道、管道→WS）加方法拦截 |
| `playground/src/protocol.rs` | `LspTokens`、`LspDiagnostics`、`LspDiagnosticItem`、`LspCompletion`、`CompletionItem`、`CompletionLabelDetails`、`ClientMsg::LspChange`、`ClientMsg::LspCompletion` |
| `docs/typst/play/lsp-client.js` | `onMessage` 的三种消息分支、`_applyTokens` / `_setDiagnostics` / `_resolveCompletion` 的消息耦合；换成 JSON-RPC 层 + 主动请求 |
| `playground/src/lsp/service.rs` | `write_source` 若只服务 LSP 路径则删除；切换为在 `didOpen` / `didChange` 拦截处落盘 |
| 测试 | 围绕 legend 重映射、`unchanged` 回放、`request_id` 配对的测试随实现一起删；新增转发与握手测试 |

保留不动：`frame.rs`、`docker_exec.rs`、`exec.rs`、`pool.rs`。`service.rs` 只改就绪判据（见上），容器与会话的其余管理不变。

## 错误处理

- RA 管道断开 → 服务端发一条合成的 `exit` 通知，归还会话时按死会话销毁（现有 `ws.rs:368-372` 的逻辑不变）。
- 前端在收到 `exit` 后：清 tokens / diagnostics、把 pending 全部 reject、显示中性提示；下一次编辑走重连。
- 单条请求不设全局超时，只有补全保留 2 秒兜底；`exit` 是唯一的统一取消信号。
- 服务端写管道失败（容器没了）按 `exit` 处理，不再尝试重连同一个会话。

## 测试

Rust（`playground/src/lsp/session.rs` / `server/ws.rs` 的现有假 docker harness）：

1. 浏览器 `initialize` 得到的是待命阶段缓存的 capabilities，且没有写进 RA 管道；
2. `initialized` 被吞掉；
3. `didOpen` 的文本落到磁盘，并且原样进管道；
4. `didChange` 的全文落盘并原样进管道；
5. 任意未知请求（例如 `textDocument/definition`）原样往返，`id` 不变；
6. RA 发来的通知原样到前端；
7. 交棒前有 `didClose`，新会话的 `didOpen` 合法；
8. `shutdown` / `exit` 不被转发；
9. 管道断开产生合成的 `exit` 通知，会话不还池；
10. 就绪判据仍能在 `lsp_boot_timeout` 内等到诊断响应，超时会话被丢弃。

前端（手工 + Playwright）：

1. 打开页面后高亮与诊断出现；
2. 编辑后 300 ms 内重取；
3. 补全弹窗仍按现有格式显示；
4. 断线重连后高亮恢复；
5. 顺手用一条临时 binding 验证 `textDocument/definition` 能往返（不进入 UI，只证明传输通了）。

## 迁移步骤

1. `protocol.rs` 加信封与 `LspReady.uri`，抬协议版本；
2. `session.rs` 拆出「转发」与「待命」两条路，删翻译层；缓存 `initialize` 响应；
3. `ws.rs` 换成双泵 + 方法拦截 + `didClose` 边界；
4. 前端换传输层，tokens/诊断改成主动请求；
5. 删除旧消息、旧测试；
6. 文档站更新「RA 会话」一节（`docs/typst/arch/play.typ`）。

## 风险

- **RA 的 workspace 解析依赖磁盘**：`didOpen` / `didChange` 的落盘是必须的，不能省。落盘失败要明确报错，不能只发 LSP。
- **`didClose` 之后 crate graph 是否仍保留**：大概率保留；若实测掉得厉害，退回「把新会话第一条 `didOpen` 改写成 `didChange`」的方案，服务端多一个状态位。
- **浏览器 capability 不生效**：RA 的响应是按服务端待命时的 capability 生成的。若以后要开新的 capability（例如 pull diagnostics 的 `relatedDocuments`），要改的是服务端待命时的 `initialize` 参数。
- **前端要应答 RA 的请求**：漏答会让 RA 挂起。统一回 `null` 是兜底。
