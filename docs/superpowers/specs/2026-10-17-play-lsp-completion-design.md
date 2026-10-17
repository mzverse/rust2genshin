# 摆弄页补全（容器内 rust-analyzer）设计

日期：2026-10-17
状态：待实现

## 目标

摆弄页（`docs/typst/play/`）的编辑框目前是裸 `<textarea>`，只有「编译 → 看日志」。要给它加
**语法高亮、诊断、补全**：用户写 `rust2genshin_lib::sc::` 就能列出模块里的函数名，函数名同时
被染色。

三样**全部由容器里的 rust-analyzer 通过 LSP 提供**，前端不做任何 Rust 解析。

## 非目标

- 不做类型推导展示、不做 hover 文档浮窗。LSP 能给，但前端工作量另算。
- 不做多文件编辑。始终只编辑一个虚拟文件 `demo/src/lib.rs`（`protocol::SOURCE_FILE`）。
- 不做跨会话共享 RA。一个会话一个 RA 进程（理由见「与 exec 传输的耦合」）。
- 不把 RA 结果写进 `.gia` 或编译产物。补全与编译是两条独立的路。

## 传输层：为什么是 `docker exec -i`

LSP 的 stdio 传输是 `Content-Length: <n>\r\n\r\n<json>`。`runner.rs` 现在的做法是把
`docker run` 的 stdout 当容器输出读，那条路上套着 docker CLI 的 **8 字节复用头**
（`docker.rs:68-92` 的 `FrameDecoder`）——它区分 stdout/stderr，服务无法解析
`Content-Length` 帧。

`docker exec -i` 走的是另一条路径，宿主机到容器是**裸字节管道**，不套那个头。

实测（服务端 `rust2genshin-play:6bf476b` 镜像，nightly-2026-10-08 的
`rust-analyzer 1.101.0-nightly (1d81eb4 2026-10-07)`）：

```
$ printf 'Content-Length: 92\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"initialize",
         "params":{"capabilities":{},"processId":null}}' \
  | docker exec -i <cid> sh -c 'rust-analyzer'
0000000   C   o   n   t   e   n   t   -   L   e   n   g   t   h   :    
0000020   2   7   9   8  \r  \n  \r  \n   {   "   j   s   o   n   r   p
```

回包头完好，`docker exec -i` 不需要任何拆帧胶水。

### 已排除：容器 IP 直连

默认 bridge 的容器确实有 `172.17.0.x`、宿主机也能路由过去——连通性没问题。缺的是
**监听方**：这个版本的 rust-analyzer 没有任何网络选项（`--help` 只有无网络的默认模式和
一个 `parse` 子命令，**没有 `lsp-proxy`**），它只从 stdin 读、往 stdout 写，不 bind 任何端口。

走 IP 就得在容器里补一个 TCP↔stdio 的翻译器（`socat` 不认 `Content-Length` 帧，得手写）。
`docker exec -i` 做的是同一件事，只不过这段翻译 docker 已经实现好了。

除非将来要「多个会话共享一个 RA 进程」，否则没有理由选 IP。

## 待命池把管道一起待命

RA 一旦跑起来，它的 stdio 就绑死在当初拉起它的那条管道上，后来的 `docker exec -i`
**接不进去**。但这不构成障碍——**拉起它的是服务自己**，服务可以一直攥着那个 exec
子进程和它的 stdin/stdout 句柄不放：

- 待命时：`docker run -d` 起容器 → 立刻 `docker exec -i` 起 RA → 驱动它走完
  `initialize` / `initialized` / `didOpen` → RA 把 crate graph 拉完 → **管道保持开着**，
  标记为就绪；
- 会话来了：直接把消息写进**这条已经开着的管道**，不新建 exec。RA 的 crate graph 已经是热的，
  6–11s 的冷启动整段从用户的关键路径上消失；
- 会话结束：`didClose`（或留着一个 stale 的 didOpen，见下），管道**不关**，容器回池；
- 回收：`docker rm -f` 同时干掉 exec 和 RA。

所以「接不上」的准确表述是「**新 exec 接不上**」，不是「热不起来」。池预热到的是
「容器在 + RA 在 + crate graph 已加载 + 管道开着」。

### 待命 RA 的内存代价（实测）

| 阶段 | RSS |
|---|---|
| 刚起，未 `initialize` | 29 MB |
| `initialize` 后 | 158 MB |
| `didOpen`（拉 crate graph）后 4s | 755 MB |
| +12s 稳定 | **839 MB** |

部署机 15 GiB 内存、**0 swap**、当前可用 7.8 GiB。一个热 RA 是 839 MB，约为一个编译任务
容器默认配额（`--memory 1536m`）的 55%，而后者只在编译的几秒里活着，前者是常驻的。

`POOL_MIN = 1` 稳妥。**不建议按 `MAX_JOBS` 等量预热**：n=3 就是 2.5 GB 常驻，而默认
`PLAYGROUND_MAX_JOBS = 3`。要跟着涨就调 `POOL_MIN`，不要让池自动等于上限。

**RA 容器必须下 `--memory`**（默认 2g）。没有 swap 的话，RA 膨胀到 cgroup 上限时的后果是
内核直接 OOM kill —— 用户的会话会在敲键盘的间隙当场断掉。限额把它变成一次可归因的失败，
而不是随机断线。CPU 给 `--cpus 2`：RA 是在人敲键盘的间隙跑的，压狠了补全会迟钝。

### 就绪信号与存活性

待命容器不能只看「进程起来了」——`initialize` 返回时 crate graph 还没加载完。
判据用 **`demo/src/lib.rs` 的 `publishDiagnostics` 到达**：这份诊断必然发生在 RA 把该 crate
解析进 graph 之后（此时内容可能是空的，无所谓，待命期间收到的诊断一律丢弃）。没等到就在
`PLAYGROUND_LSP_BOOT_TIMEOUT`（默认 30s）后判这个容器不合格，直接丢弃重来。

取用时还要做一次**存活性检查**：往管道里发一个廉价请求（比如
`$/memoryUsage`，或任意一个自定义 id 的 ping），超时没回就说明这个热 RA 卡死了 —— 不能
把它交给用户。检查失败就丢弃该容器、从池里再取一个。

### 跨会话复用同一个 URI

同一个 RA 连着服务多个会话时，`demo/src/lib.rs` 这个 URI 是复用的。因为每次 `LspChange`
都是**全文替换**，内容不会串；而且复用 URI 让 RA 保住已有的分析结果，比换 URI 重解析更快。

代价是上一个会话的文档状态会留在 RA 里（若它没走 `didClose`）。全文替换已经覆盖了这一点，
不必为它单独清理。

## Workspace：只分析三个成员

RA 的分析范围由 `Cargo.toml` 决定，不由打开哪个文件决定。实测裁剪效果：

| workspace | 成员 | 冷启动 | 第二次 |
|---|---|---|---|
| 全量 | 7 个 | 84s | 69s |
| 裁过一轮 | demo / lib / lib-internal / **core** / build-demo | 28s | 13s |
| **本设计** | **demo / lib / lib-internal** | **11s** | **6s** |

关键不是「裁了几个」，是**裁掉的是谁**：

- `core` 是 rustc codegen 后端，`#![feature(rustc_private)]` + 一堆 `extern crate rustc_*`。
  为了能解析它，RA 要把整个编译器 sysroot 的元数据拉进来分析——84s→28s 的大头就在这。
- `build-demo` 只是驱动 `core` 的脚本，自身零成本，但它把 `core` 拖进来。
- `demo → lib → lib-internal` 这条链上**根本没有 core**：`lib-internal` 只依赖
  `syn`/`quote`/`proc-macro2`/`zyn`，`lib` 只依赖 `lib-internal`。对「给 demo 的源码做补全」
  这件事，`core` 从一开始就是无关的。

剩下 187 个包几乎全是 proc-macro 那几棵依赖树的传递闭包。

**结论**：RA 容器只挂 `demo/src/`、`lib/`、`lib-internal/`、`Cargo.lock`、
`rust-toolchain.toml`，以及一份**裁过的 `Cargo.toml`**（`members` 与 `default-members`
都只留这三项，其余照抄原文件——手写会漏掉 `workspace.package.authors` 之类的继承项，
`cargo metadata` 会静默失败）。

这同时是安全收敛：容器里没有后端、没有 viewer、没有文档构建脚本。

### RA 缓存烘不进镜像

RA 的 salsa 查询缓存严格在进程内。`prime-caches` 不落盘、`--persistent-query-serialization`
不存在、`~/.cache` 从不出现。所以上面那 6–11s 每个新进程都要重付，只能靠池化摊薄。

## 容器池

### 语义

- **下限**：空闲**已就绪**容器数不低于 `PLAYGROUND_LSP_POOL_MIN`（默认 **1**）。服务确保随时
  有这一个容器在位待命。
- **上限**：并发 RA 会话数 = `PLAYGROUND_MAX_JOBS`。
- **回收**：会话释放 → 容器回池 → 若此时空闲数 **超过** `POOL_MIN`，才 `docker rm -f` 掉超出
  的部分。

> **Ruling**：用户原话是「空余容器达到 n 也才真正释放容器」。按字面取「达到 n 即销毁」，第一次
> 释放就会把池打到 `n-1`，待命就没了，与前半句「预留 n 个待命」自相矛盾。故取**下限语义**：
> 销毁发生在空闲数**超过** n 时。

### 补池的时机

池的补齐是**后台**的，不能挡在用户前面：

- 服务起来时先拉起一个；
- 一个就绪容器被取走后，后台立刻开始补下一个（下限是 1，不是 2）；
- 待命容器在 `POOL_MIN` 被抬高、或当前空闲数已达标时，才停止补。

一个待命容器从零到就绪是「docker run ~1s + RA 冷启动 6–11s」，全程在后台，不计入任何用户
请求的延迟。

### 空闲回收

- `PLAYGROUND_LSP_IDLE_TTL`，默认 **90s**。0 = 永不回收。
- 因为待命是热的（管道开着、graph 已加载），这个 TTL 纯粹是**内存策略**，不是性能策略：
  90s 没人用就省下 839 MB，有人来了再花 6–11s 重热。取值权衡的是内存与重建延迟。
- 回收动作 = `docker rm -f`，同时干掉 exec 和 RA，不留残容器。`gc_loop.rs` 已有类似的
  周期任务骨架。

### 命名

`r2g-ra-<n>`，`n` 来自一个单调计数器（复用 `JobPaths` 那种自增 id 的做法）。
`exec` 时用容器名而不是容器 id——名字稳定，容器重建后不需要重新发现。

## 会话生命周期

```
(后台，与用户无关)
  └─ 补池 → docker run -d 起容器 → docker exec -i 起 RA → initialize/initialized
       → didOpen(占位内容) → 收到 demo/src/lib.rs 的 publishDiagnostics
       → 标记就绪，管道保持开着

前端 WebSocket 建立
  └─ hello 里带 lsp_supported                    ← 前端据此决定要不要挂 LSP 客户端
用户首次编辑 / 聚焦编辑框
  └─ ClientMsg::LspOpen
       ├─ 取一个就绪容器；池空且未达上限 → 现建一个(用户要等 6–11s)
       ├─ 存活性检查(超时则弃用该容器)
       ├─ 写入用户的当前源码到容器内 demo/src/lib.rs
       ├─ textDocument/didOpen(内容 = 当前源码)  ← 复用同一 URI
       └─ workspace/didChangeWatchedFiles(把 demo/src/lib.rs 标脏)
每次编辑(去抖 300ms)
  └─ ClientMsg::LspChange { source }
       └─ textDocument/didChange(全文替换)
每次按键后
  └─ 后端在 RA 侧收 publishDiagnostics → 转发成 ServerMsg::LspDiagnostic
前端 WebSocket 关闭
  └─ 容器回池(管道不断)→ 按下限规则决定是否 docker rm -f
池内空闲超过 PLAYGROUND_LSP_IDLE_TTL
  └─ docker rm -f
```

注意 `LspOpen` 那一步**通常不付冷启动**：池里已经有一个就绪容器在等着。付出 6–11s 的只有
「池空 + 正在现建」这一种情况（比如服务刚起来、或刚被一次回收打空）。

### 源码怎么进容器

用户改的是浏览器内存里的文本，容器里那份是启动时写进去的旧值。每个会话由**前端负责**
把当前完整源码发上来（`LspChange` 带全文，不做前端增量——单文件、大小上限 1 MiB
（`protocol::MAX_SOURCE_BYTES`），整传比增量省事且不会算错偏移），后端
`docker exec -i <cid> sh -c 'cat > demo/src/lib.rs'` 覆盖写，再发 `didChange`。

`didChangeWatchedFiles` 仍然要发：RA 会用它去重解析文件、加进 crate graph。

### 增量 vs 全量

LSP 的 `didChange` 规范上支持 `range` 增量。这里**用全量替换**（不带 `range`），
理由：单文件、文本不大、服务端不持有前端光标，实现增量要引入版本号对齐和偏移换算，
换来的只是几百字节带宽。不划算。

## 协议扩展

`protocol.rs:3-5` 已经把「将来加 `{"type":"lsp"}` 双向转发」写进注释了，照那个预留走。

`ClientMsg` 加：

```rust
LspOpen {},
LspChange { source: String },
```

`ServerMsg` 加：

```rust
LspReady { file: String },
/// RA 的 semanticTokens delta 数组，原样透传。**不在服务端拆** —— 拆成
/// (start, len, type, modifiers) 是 CodeMirror 扩展的活，服务只搬运。
LspTokens { data: Vec<u32> },
LspDiagnostic { line: u32, column: u32, end_line: u32, end_column: u32,
                severity: DiagnosticSeverity, message: String },
LspCompletion { request_id: u32, items: Vec<CompletionItem> },
```

`CompletionItem` 只需 `{ label, kind, detail }` 三个字段——够前端渲染列表；`insertText`
先不做（RA 默认给的 `insertText` 与 `label` 一致）。

`Hello` 加 `lsp_supported: bool`，让前端在旧后端上优雅降级（不挂 LSP 客户端，退回今天的行为）。

`PROTOCOL_VERSION` 从 1 抬到 2。

### 补全请求的方向

LSP 的 completion 是**请求-应答**（`textDocument/completion` → 带 id 的响应），而
`ServerMsg` 是单向广播流。这里用 `LspCompletion { request_id }` 让前端把自己的 id 原样
回显，服务端按 id 配对。多客户端时 id 由前端自行保证唯一。

## 前端：编辑功能全部来自 RA

编辑器换 CodeMirror 6，但**不引入任何 Rust 解析器** —— `@codemirror/lang-rust`、
tree-sitter、Lezer 语法表统统不要。语法高亮、诊断、补全三样都是 RA 通过 LSP 给的：

| 能力 | 来源 | CodeMirror 侧 |
|---|---|---|
| 语法高亮 | `textDocument/semanticTokens/full` | 自写一个 `StateField` + `Decoration`，把 RA 的 delta 数组映射成 range 上的样式 class |
| 诊断 | `textDocument/publishDiagnostics` | 自写一个 `StateField` + `Decoration`，或直接借用 `@codemirror/lint` 的 `setDiagnostics` |
| 补全 | `textDocument/completion` | 自写 `CompletionSource` |

实测这个 RA 版本（`1.101.0-nightly`）**给 `semanticTokensProvider`**，
`textDocument/semanticTokens/full` 也真的返回 delta 编码的 token 数组 —— 前提是客户端在
`initialize` 的 `capabilities.textDocument.semanticTokens` 里声明支持。这条要在
`ServerMsg` 里加一个 `LspTokens { data: Vec<u32> }` 透传（不要在前端拆 delta 再转发，
拆分是 CodeMirror 扩展的事，服务只做透传）。

好处是本地零解析：没有 Lezer 语法表要下载、没有 wasm 要等、不存在「高亮和分析结论不一致」。
代价是高亮要等 RA 就绪才出现（待命池的意义之二），以及 RA 卡住时高亮会一起消失。

依赖的 npm 包因此缩到 `@codemirror/state`、`@codemirror/view`、`@codemirror/autocomplete`
（`@codemirror/lint` 可选）。挂 npm 包意味着摆弄页不再是无构建的静态页 —— 这是本设计
**唯一的前端成本**。

### 高亮的空窗期

在 RA 就绪之前，编辑框是纯文本。UI 上用一条提示说「分析中…」，而不是留一片纯黑等它。
拿到第一个 `LspTokens` 后提示消失。

## 配置

| 变量 | 默认 | 含义 |
|---|---|---|
| `PLAYGROUND_LSP_POOL_MIN` | 1 | 空闲容器数下限 |
| `PLAYGROUND_LSP_IDLE_TTL` | 90s | 空闲多久回收（0 = 永不） |
| `PLAYGROUND_LSP_BOOT_TIMEOUT` | 30s | 从 `LspOpen` 到 `LspReady` 的上限 |
| `PLAYGROUND_LSP_DISABLED` | false | 一键关掉整条链路（回退到今天的行为） |

全部走 `config.rs` 现有的 `raw()` / `num_var()` 解析，默认值只写在 `Config` 里一处。

## 镜像

`core/proto` 无关。RA 需要在**任务镜像之外**单独一份带组件的镜像：`rust-analyzer`
组件约 +41 MB（`.rustup` 1552 MB → 1593 MB）。若塞进任务镜像，每次编译都白背这 41 MB。

做法：`image_builder.rs` 的 Dockerfile 加一个 `FROM <play-image> AS ra` 的 stage，只做
`rustup component add rust-analyzer`，最后打两个 tag。`Baker` 现在只认一个
`serving` 镜像（`bake.rs:154` 的 `BakeStatus`），要扩成认两个。

镜像源沿用现有的 `rustup_dist_server` / `cargo_mirror`（服务端已配 rsproxy）。

## 测试

`runner.rs` / `docker.rs` 都有成片的 `#[cfg(test)]`，照它们的路子：

- `docker::exec_args` —— 断言参数是 `exec -i <name> sh -c <cmd>`，`-i` 在、
  `-t` 不在（`-t` 会把裸管道变成带 `\r\n` 的伪终端，拆帧又废了）。
- `lsp::FrameDecoder` —— 拆 `Content-Length` 帧；头跨 chunk、body 跨 chunk、
  多个帧在一个 chunk 三种切法。
- `lsp::Pool` —— 池下限/上限/回收的纯状态机测试（用假容器，不碰真 docker）：
  取空池 → 建一个；再取 → 建第二个；放回 → 空闲 2 > 1 → 销毁一个；再取 → 复用。
  外加：取用时存活性检查失败 → 该容器被丢弃且**不**退回池。
- `protocol` —— 新消息的 serde 往返。
- 集成（`testing.rs` 里那套）：真起一个容器，走完 `LspOpen` → `didOpen` →
  `didChange` → 收到 `LspDiagnostic`；再走一遍「待命 → 复用同一个 RA 跨两个会话」，
  断言第二个会话拿到的是**同一个** RA 进程（对比 `/proc/<pid>` 或 RA 报的 pid）。

## 风险

- **补全质量依赖 RA 看到的是**用户的**当前**源码。若 `LspChange` 丢了一次，RA 会对着旧
  内容给补全，表现为「补了个不存在的东西」——比没有补全更误导。宁可延迟发也不要发错：
  `LspChange` 必须整串替换并 await 写盘成功再发 `didChange`。
- **容器里没有 core**，所以 RA 不认识 `core` 里的类型。`demo` 源码不该引用它们，引用了
  RA 报 unresolved——这与真实编译一致，可以接受。
- **内存是本设计的主要成本**，不是 CPU。一个热 RA 常驻 839 MB，部署机只有 15 GiB 且无 swap。
  把 `POOL_MIN` 调大前先算这笔账。
- **待命容器长期持有未关闭的 exec 管道**。若服务的 exec 子进程泄漏（`Child` 被 drop 而没
  `kill_on_drop`），docker exec 进程会一直占着容器里的槽位。要么给 exec 子进程设
  `kill_on_drop(true)`，要么在 `Drop` 里补 `docker rm -f`——`runner.rs:129-185` 的
  `ContainerGuard` 就是这个模式的现成写法，照抄一份给 RA 池。
- **RA 卡死会把坏管道交给下一个用户**。存活性检查（见上）是唯一的防线，不能省。
