# 摆弄页补全（容器内 rust-analyzer）设计

日期：2026-10-17
状态：待实现

## 目标

摆弄页（`docs/typst/play/`）的编辑框目前是裸 `<textarea>`，只有「编译 → 看日志」。要给它加
Rust 补全与诊断：用户写 `rust2genshin_lib::sc::` 就能列出模块里的函数名。

配套的前置项是**语法高亮**——它不依赖容器，是独立的、可先落地的部分。本设计只谈补全。

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

## 与 exec 传输的耦合（决定了池的形态）

RA 一旦在容器里跑起来，它的 stdio 就绑在当初拉起它的那条管道上。后来的
`docker exec -i` **接不进一个已在运行的 RA 的 stdio**。所以：

- 会话开始时，在**空闲容器**上 `docker exec -i` 起 RA；
- 会话结束时，这个 exec 进程被杀掉，RA 随之死；
- 容器回到空闲池，但**它没被预热过**——下次会话仍要重付 RA 启动开销。

这条约束是「待命池」能预热到的上限，别在文档或 UI 里把它写成「秒开」。

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

- **下限**：空闲容器数不低于 `PLAYGROUND_LSP_POOL_MIN`（默认 **1**）。会话未占满时，服务
  保证有这一个容器在位待命。
- **上限**：并发 RA 会话数 = `PLAYGROUND_MAX_JOBS`。
- **回收**：会话释放 → 容器回池 → 若此时空闲数 **超过** `POOL_MIN`，才 `docker rm -f` 掉超出
  的部分。

> **Ruling**：用户原话是「空余容器达到 n 也才真正释放容器」。按字面取「达到 n 即销毁」，第一次
> 释放就会把池打到 `n-1`，待命就没了，与前半句「预留 n 个待命」自相矛盾。故取**下限语义**：
> 销毁发生在空闲数**超过** n 时。

### 池是插补，不是预热

容器创建（`docker run -d`）约 1s。**空闲时不预先起 RA**（起了也接不上，见「耦合」一节），
池里就是「容器在、没进程」。

### 空闲回收

- `PLAYGROUND_LSP_IDLE_TTL`，默认 **90s**。0 = 永不回收。
- 上限不是拍出来的：RA 重热要 6–11s（见上表），窗口太短会让每次会话都白等一次分析。
- 回收动作 = `docker rm -f`，不留残容器。`gc_loop.rs` 已有类似的周期任务骨架。

### 命名

`r2g-ra-<n>`，`n` 来自一个单调计数器（复用 `JobPaths` 那种自增 id 的做法）。
`exec` 时用容器名而不是容器 id——名字稳定，容器重建后不需要重新发现。

## 会话生命周期

```
前端 WebSocket 建立
  └─ hello 里带 lsp: {supported: true}          ← 前端据此决定要不要挂 LSP 客户端
用户首次编辑 / 聚焦编辑框
  └─ ClientMsg::LspOpen                          ← 后端取一个空闲容器(或新建)
       ├─ docker exec -i 起 rust-analyzer
       ├─ LSP: initialize / initialized
       ├─ 写入用户的当前源码到容器内 demo/src/lib.rs
       ├─ textDocument/didOpen(内容 = 当前源码)
       └─ workspace/didChangeWatchedFiles(把 demo/src/lib.rs 标脏)
每次编辑(去抖 300ms)
  └─ ClientMsg::LspChange { source }
       └─ textDocument/didChange(增量或全量)
每次按键后
  └─ 后端在 RA 侧收 publishDiagnostics → 转发成 ServerMsg::LspDiagnostic
前端 WebSocket 关闭 / 空闲超时
  └─ 杀掉 exec 进程(RA 随之死)→ 容器回池 → 按上限规则决定是否 docker rm -f
```

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

## 前端

编辑器换 CodeMirror 6（`@codemirror/lang-rust` 给语法高亮，`@codemirror/lint` 接诊断，
自己写一个 source 扩展接 completion）。理由是原生 `<textarea>` 没有任何扩展点。

挂 npm 包意味着摆弄页不再是无构建的静态页——这是本设计的**最大前端成本**，也是语法高亮
单独先做的原因：高亮那一档可以只上 `@codemirror/lang-rust`，不碰后端。

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
- `protocol` —— 新消息的 serde 往返。
- 集成（`testing.rs` 里那套）：真起一个容器，走完 `LspOpen` → `didOpen` →
  `didChange` → 收到 `LspDiagnostic`。

## 风险

- **补全质量依赖 RA 看到的是**用户的**当前**源码。若 `LspChange` 丢了一次，RA 会对着旧
  内容给补全，表现为「补了个不存在的东西」——比没有补全更误导。宁可延迟发也不要发错：
  `LspChange` 必须整串替换并 await 写盘成功再发 `didChange`。
- **容器里没有 core**，所以 RA 不认识 `core` 里的类型。`demo` 源码不该引用它们，引用了
  RA 报 unresolved——这与真实编译一致，可以接受。
- **待命容器占内存**。空闲容器只是 `sleep`，占用很小；但 RA 起来后是几百 MB 量级。
  容器要不要下 `--memory` / `--cpus` 限额：RA 是在人敲键盘的间隙跑的，超限会让 RA 被 OOM
  kill。建议下 `--cpus 2`，`--memory` 先不给（与服务端 cgroup 的已知限制一致，见
  `config.rs::memory_var` 的注释）。
