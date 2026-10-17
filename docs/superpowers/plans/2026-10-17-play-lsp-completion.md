# 摆弄页补全与高亮 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 摆弄页的编辑框换成 CodeMirror 6，语法高亮 / 诊断 / 补全三样全部由容器里的 rust-analyzer 经 LSP 提供。

**Architecture:** 服务端新增 `lsp` 模块，负责 `Content-Length` 拆帧、`docker exec -i` 起的 RA 管道、会话池、以及 LSP 会话本身；WebSocket 协议加四种消息透传。前端只做 RA 数据到 CodeMirror 状态的映射，不含任何 Rust 解析。

**Tech Stack:** Rust / tokio / axum（服务端）；rust-analyzer 1.101.0-nightly（LSP）；CodeMirror 6 via esm.sh（前端）。

**Spec:** `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md` —— plan 与 spec 冲突时以 spec 为准。

## Global Constraints

- **不改 `docker run` 那条路。** `runner.rs` / `FrameDecoder`（8 字节复用头）原样保留，它服务的是编译任务。RA 走 `docker exec -i`，那条路是裸字节管道。
- **`docker exec` 必须带 `-i`，不得带 `-t`。** `-t` 会分配伪终端、输出带 `\r\n`，拆帧又废了。
- **RA 容器里只分析 `demo` / `lib` / `lib-internal` 三个成员。** 不挂 `core`——它是 `rustc_private` 后端，会拖整个 sysroot 进分析（84s vs 11s）。
- **待命池下限默认 1，空闲数超过下限才 `docker rm -f`。**
- **待命是热的**：管道开着、crate graph 已加载。冷启动 6–11s 不能出现在用户关键路径上。
- **RA 容器限额 `--memory 2g --cpus 2`。** 部署机 15 GiB、0 swap，超限是内核 OOM kill。
- **所有注释用中文**，风格照抄邻近文件（`runner.rs` / `queue.rs`）。
- **不改 `docs/typst/lib/` 下除 `play/` 外任何文件。**
- **提交时逐个 `git add <file>`，禁止 `git add .` / `git add -A`。**

## Review Focus

这些是 spec 蕴含、但没有哪条测试会自然覆盖的失败模式：

1. **RA 看到的是旧源码** —— 用户改了代码，RA 补全里还建议着上一版才有的名字。表现为「补了个不存在的东西」，比没有补全更误导。测试：先编入一个函数名，改掉它（300ms 去抖之后），断言新补全里**不再**有旧名字。
2. **池空时把没就绪的容器交给用户** —— 用户等 12 秒后才看到高亮，且没有任何解释。测试：杀掉一个待命 RA 的进程（留容器），断言取用走的是另一个而不是它。
3. **待命池吃掉全部内存** —— `POOL_MIN` 被调成等于 `MAX_JOBS=3` 时常驻 2.5 GB。测试：断言池的空闲数**从不**低于 `POOL_MIN`，也**从不**高于 `POOL_MIN + 在用数`。
4. **exec 子进程泄漏** —— 服务 abort 会话时忘了杀 exec，容器里的 RA 一直占着 839 MB。测试：断开 WebSocket 后断言容器内 RA 进程消失。
5. **高亮比诊断先到（或反之）** —— 前端若把 `LspTokens` 当诊断用，用户会看到一堆假错误。测试：`LspTokens` 与 `LspDiagnostic` 分别渲染到不同的 DOM 层，断言两者可同时存在。
6. **协议版本没抬** —— 老前端连上新后端仍发 v1 消息，服务按 v1 解析会静默丢掉 LSP 分支。测试：`Hello` 带 `lsp_supported: true` **且** `protocol == 2`。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `playground/src/lsp/mod.rs` | 模块声明 + 公共类型出口 |
| `playground/src/lsp/frame.rs` | `Content-Length` 拆帧 / 组帧，零 I/O、纯函数 |
| `playground/src/lsp/exec.rs` | `docker exec` 参数构造 + RA 进程拉起，抽象成 trait 以便测试 |
| `playground/src/lsp/session.rs` | 一个 RA 会话：initialize / didOpen / didChange / 收诊断与 token |
| `playground/src/lsp/pool.rs` | 容器池状态机（下限、上限、回收、存活性） |
| `playground/src/docker.rs` | **新增** `exec_args`、`run_detached_args`（不动既有 `run_args`） |
| `playground/src/protocol.rs` | 新消息 + `PROTOCOL_VERSION` 抬到 2 |
| `playground/src/config.rs` | 新增 5 个 `PLAYGROUND_LSP_*` |
| `playground/src/server/ws.rs` | 分发 `LspOpen` / `LspChange`，把 RA 来的事件转成 `ServerMsg` |
| `playground/src/server/mod.rs` | `AppState` 加 `lsp: Option<Arc<LspPool>>` |
| `playground/src/image_builder.rs` | Dockerfile 加 `ra` stage |
| `playground/src/main.rs` | 装配池、后台补池、后台回收 |
| `docs/typst/play/index.typ` | 编辑框换成挂载点 + module script |
| `docs/typst/play/script.js` | 改成 ESM，装 CodeMirror + 三个 RA 扩展 |
| `docs/typst/play/lsp-client.js` | 新增：WS ↔ LSP 的桥，纯前端无依赖 |

---

### Task 1: `Content-Length` 拆帧

**Files:**
- Create: `playground/src/lsp/frame.rs`
- Create: `playground/src/lsp/mod.rs`
- Modify: `playground/src/main.rs`（加 `mod lsp;` —— 这个 crate 只有 main.rs，没有 lib.rs，不在这里声明模块，测试 runner 根本扫不到）

**Interfaces:**
- Produces: `frame::FrameDecoder`（`new()` / `push(&[u8]) -> Result<Vec<Vec<u8>>, FrameError>`）、
  `frame::encode(msg: &str) -> Vec<u8>`、`frame::FrameError`。

`FrameDecoder::push` 收任意切片的字节流，返回**完整消息体**（不含头）。

- [ ] **Step 1: 写失败的测试**

在 `frame.rs` 底部写：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ok(m: &str) -> Vec<u8> {
        encode(m)
    }

    #[test]
    fn one_frame_whole() {
        let mut d = FrameDecoder::new();
        assert_eq!(d.push(&ok("hello")).unwrap(), vec![b"hello".to_vec()]);
    }

    #[test]
    fn header_split_across_chunks() {
        let full = ok("hello");
        let mut d = FrameDecoder::new();
        assert!(d.push(&full[..5]).unwrap().is_empty());
        assert!(d.push(&full[5..15]).unwrap().is_empty());
        assert_eq!(d.push(&full[15..]).unwrap(), vec![b"hello".to_vec()]);
    }

    #[test]
    fn two_frames_one_chunk() {
        let mut chunk = ok("a");
        chunk.extend(ok("bb"));
        let mut d = FrameDecoder::new();
        assert_eq!(
            d.push(&chunk).unwrap(),
            vec![b"a".to_vec(), b"bb".to_vec()]
        );
    }

    #[test]
    fn utf8_body_survives_split() {
        let full = encode("中文字符串");
        let mut d = FrameDecoder::new();
        let mut out = Vec::new();
        for i in 0..full.len() {
            out.extend(d.push(&full[i..i + 1]).unwrap());
        }
        assert_eq!(out, vec!["中文字符串".as_bytes().to_vec()]);
    }

    #[test]
    fn encode_is_byte_length_not_char_length() {
        // 中文一个字 3 字节,头里写 9 而不是 3
        assert!(encode("中").starts_with(b"Content-Length: 3\r\n"));
    }

    #[test]
    fn bad_header_is_an_error() {
        let mut d = FrameDecoder::new();
        assert!(matches!(
            d.push(b"Content-Length: abc\r\n\r\n{}"),
            Err(FrameError::BadHeader(_))
        ));
    }

    #[test]
    fn body_shorter_than_declared_is_incomplete_not_an_error() {
        let full = encode("hello");
        let mut d = FrameDecoder::new();
        assert!(d.push(&full[..full.len() - 2]).unwrap().is_empty());
    }
}
```

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground lsp::frame
```
Expected: 编译失败 —— `cannot find function encode` / `cannot find struct FrameDecoder`。

- [ ] **Step 3: 实现**

`playground/src/lsp/mod.rs`：

```rust
//! RA 会话。见 `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md`。
//!
//! 这里的东西**与编译任务那条路完全无关**:编译走 `docker run` 的 attach 协议
//! (8 字节复用头,见 `docker::FrameDecoder`),RA 走 `docker exec -i` 的裸管道。

pub mod frame;
```

`playground/src/lsp/frame.rs`：

```rust
//! LSP 的 stdio 拆帧:`Content-Length: <字节数>\r\n\r\n<正文>`。
//!
//! 与 `crate::docker::FrameDecoder` 是两回事 —— 后者解的是 docker CLI attach
//! 协议的 8 字节头。这里解的是 LSP 自己的头,两者不能混用。

/// 头找不到、长度不是数字、或头没写完就收到了正文。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("LSP 帧头不对:{0}")]
    BadHeader(String),
}

/// 组一帧。**长度按字节算**(`str::len()`),不是按字符数:UTF-8 下一个中文
/// 占 3 字节,写成字符数 RA 会截帧。
pub fn encode(msg: &str) -> Vec<u8> {
    let body = msg.as_bytes();
    let mut out = Vec::with_capacity(body.len() + 32);
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    out
}

const HEADER_END: &[u8] = b"\r\n\r\n";

#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// 喂任意一段字节,吐出里面**已经完整**的正文。
    ///
    /// 半个帧不是错误,只是还没有输出 —— 调用方拿 `Ok(vec![])` 当「还没到齐」。
    /// `BadHeader` 才是真的要断线。
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            let Some(hdr_end) = find(&self.buf, HEADER_END) else { break };
            let header = std::str::from_utf8(&self.buf[..hdr_end])
                .map_err(|e| FrameError::BadHeader(format!("头不是 UTF-8:{e}")))?;
            let len = parse_len(header)?;
            let body_start = hdr_end + HEADER_END.len();
            if self.buf.len() < body_start + len {
                break;
            }
            out.push(self.buf[body_start..body_start + len].to_vec());
            self.buf.drain(..body_start + len);
        }
        Ok(out)
    }
}

fn parse_len(header: &str) -> Result<usize, FrameError> {
    for line in header.split("\r\n") {
        if let Some(v) = line.strip_prefix("Content-Length:") {
            return v.trim().parse::<usize>().map_err(|_| FrameError::BadHeader(header.into()));
        }
    }
    Err(FrameError::BadHeader(header.into()))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
```

- [ ] **Step 4: 跑测试确认它过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground lsp::frame
```
Expected: 7 passed。

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/lsp/mod.rs playground/src/lsp/frame.rs playground/src/main.rs && git commit -m "feat(play): LSP 的 Content-Length 拆帧

编译任务走 docker run 的 attach 协议(8 字节复用头,见 docker::FrameDecoder),
RA 走 docker exec -i 的裸管道 —— 两者不能混用,这是独立的一份。

encode 的长度按字节算:UTF-8 下一个中文占 3 字节,写成字符数 RA 会截帧。
半个帧不算错,只是还没输出完;BadHeader 才真的要断线。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: `docker exec` 参数

**Files:**
- Modify: `playground/src/docker.rs`（在 `kill_args` 之后追加，不动既有函数）

**Interfaces:**
- Consumes: 无。
- Produces: `docker::exec_args(name: &str, cmd: &str) -> Vec<String>`、
  `docker::run_detached_args(spec: &RunSpec) -> Vec<String>`。

- [ ] **Step 1: 写失败的测试**

追加到 `docker.rs` 既有的 `mod tests` 里：

```rust
    #[test]
    fn exec_has_i_and_not_t() {
        let a = exec_args("r2g-ra-1", "rust-analyzer");
        assert!(a.contains(&"-i".to_string()));
        assert!(!a.contains(&"-t".to_string()));
    }

    #[test]
    fn exec_shape_is_run_subcommand() {
        let a = exec_args("r2g-ra-1", "rust-analyzer");
        assert_eq!(
            a,
            vec!["exec", "-i", "r2g-ra-1", "sh", "-c", "rust-analyzer"]
        );
    }

    #[test]
    fn run_detached_has_no_attach_flags() {
        let spec = RunSpec {
            name: "r2g-ra-1".into(),
            image: "img:tag".into(),
            src_dir: "/src".into(),
            out_dir: "/out".into(),
            workdir: "/w".into(),
            cpus: Some(2),
            memory: Some("2g".into()),
            pids_limit: 128,
            user: "1001".into(),
            source_file: "demo/src/lib.rs".into(),
        };
        let a = run_detached_args(&spec);
        assert!(a.contains(&"-d".to_string()));
        assert!(!a.contains(&"-i".to_string()));
        assert!(!a.contains(&"-t".to_string()));
        // 待命容器不跑任务,最后一段必须是常驻命令而不是 cargo
        assert_eq!(a.last().unwrap(), "sleep");
    }
```

若 `RunSpec` 字段与实际不符，按 `docker.rs:98-124` 的实际定义调整断言，但 `-i` 在 / `-t` 不在 / 以 `sleep` 结尾这三条不变。

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground docker::tests
```
Expected: 编译失败 —— `cannot find function exec_args`。

- [ ] **Step 3: 实现**

追加到 `docker.rs`：

```rust
/// RA 会话的 exec 参数。
///
/// **`-i` 必须在、`-t` 必须不在。** `-i` 开 stdin/stdout 管道 —— 而这条管道
/// 和 `docker run` 的 attach 协议不一样:exec 走的是**裸字节管道**,宿主机到
/// 容器之间不套那 8 字节复用头,所以 LSP 的 `Content-Length` 帧能原样穿过
/// (实测回包头完好)。`-t` 会分配伪终端、输出带 `\r\n`,拆帧就废了。
pub fn exec_args(name: &str, cmd: &str) -> Vec<String> {
    vec![
        "exec".into(),
        "-i".into(),
        name.into(),
        "sh".into(),
        "-c".into(),
        cmd.into(),
    ]
}

/// 待命 RA 容器:`-d` 后台,不 attach。
///
/// 末尾的 `sleep` 是必须的:容器必须有前台常驻进程才不会被判定退出
/// (镜像的 CMD 是 bash,不显式给命令的话它会起一个没有 tty 的 bash 然后
/// 立刻 exit 0 —— 看起来「起来了」,实际什么都没跑)。RA 本身由
/// `exec_args` 在这个容器里另起。
pub fn run_detached_args(spec: &RunSpec) -> Vec<String> {
    let mut a = run_args(spec);
    a.push("-d".into());
    a.push("sleep".into());
    a
}
```

- [ ] **Step 4: 跑测试确认它过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground docker::tests
```
Expected: 全绿。

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/docker.rs && git commit -m "feat(play): docker exec / 后台 run 的参数构造

exec 必须 -i 且不得 -t:exec 走裸字节管道,docker 那 8 字节复用头不套上来,
LSP 帧能原样穿过;-t 分配伪终端、输出带 \\r\\n,拆帧就废了。

待命容器以 -d 后台起,末尾显式给 sleep —— 容器得有前台常驻进程才不被判定
退出,否则镜像的 CMD(bash)会起一个没 tty 的 bash 立刻 exit 0。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: RA 进程抽象

**Files:**
- Create: `playground/src/lsp/exec.rs`
- Modify: `playground/src/lsp/mod.rs`（加 `pub mod exec;`）

**Interfaces:**
- Consumes: `crate::docker::exec_args`
- Produces:
  - `exec::RaProcess` —— 持有 exec 子进程与两端管道，`start` / `send` / `recv` / `ping` / `shutdown`
  - `exec::RaCommand` —— trait：`spawn` / `write` / `kill`，测试里用假实现

- [ ] **Step 1: 写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;

    /// 假 docker:把 spawn 记下来,write 收集起来,recv 喂预设的帧。
    #[derive(Default)]
    struct FakeDocker {
        spawned: Mutex<Vec<Vec<String>>>,
        written: Mutex<Vec<String>>,
        killed: Mutex<bool>,
    }

    #[async_trait::async_trait]
    impl RaCommand for Arc<FakeDocker> {
        async fn spawn(&self, args: &[String]) -> Result<(), exec::ExecError> {
            self.spawned.lock().unwrap().push(args.to_vec());
            Ok(())
        }
        async fn write(&self, bytes: &[u8]) -> Result<(), exec::ExecError> {
            self.written
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(bytes).into_owned());
            Ok(())
        }
        async fn kill(&self) {
            *self.killed.lock().unwrap() = true;
        }
    }

    #[tokio::test]
    async fn start_uses_exec_with_i() {
        let fake = Arc::new(FakeDocker::default());
        let p = RaProcess::start(Arc::clone(&fake) as Arc<dyn RaCommand>, "r2g-ra-1").await.unwrap();
        let spawned = fake.spawned.lock().unwrap();
        assert_eq!(spawned[0], vec!["exec","-i","r2g-ra-1","sh","-c","rust-analyzer"]);
        drop(p);
    }

    #[tokio::test]
    async fn send_frames_the_message() {
        let fake = Arc::new(FakeDocker::default());
        let p = RaProcess::start(Arc::clone(&fake) as Arc<dyn RaCommand>, "r2g-ra-1").await.unwrap();
        p.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).await.unwrap();
        let w = fake.written.lock().unwrap();
        assert!(w[0].starts_with("Content-Length: 61\r\n\r\n"), "实际:{}", &w[0][..40]);
    }

    #[tokio::test]
    async fn ping_times_out_on_silence() {
        let fake = Arc::new(FakeDocker::default());
        let p = RaProcess::start(fake as Arc<dyn RaCommand>, "r2g-ra-1").await.unwrap();
        assert!(p.ping(std::time::Duration::from_millis(50)).await.is_err());
    }
}
```

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground lsp::exec
```
Expected: 编译失败 —— `cannot find trait RaCommand`。

- [ ] **Step 3: 实现**

`exec.rs` 的核心：把「起 exec 子进程」和「和 RA 说话」拆开。真实实现用
`tokio::process::Command` + `Stdio::piped()`；`recv` 用 `tokio::select!` 在「读管道」和
「超时」之间选。注意 `recv` 不能拿 `&self` 直接读 `ChildStdout`——那要 `&mut`，所以
`stdin` 用 `Mutex<ChildStdin>`、`stdout` 用 `Mutex<ChildStdout>`，`recv` 是 `&self`。

`RaProcess` 的 `Drop` 必须杀掉子进程（`kill_on_drop(true)` + 显式 `kill`），否则
会话 abort 时 RA 会留在容器里占 839 MB —— 对应 Review Focus 第 4 条。

- [ ] **Step 4: 跑测试确认它过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground lsp::exec
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/lsp/mod.rs playground/src/lsp/exec.rs && git commit -m "feat(play): RA 进程的 exec 抽象

把「起 docker exec 子进程」和「和 RA 说话」拆开,测试才能不碰真 docker。
真实实现用 tokio::process::Command + Stdio::piped()。

Drop 里必须杀掉子进程:会话被 abort 时 RA 会留在容器里占 839 MB。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: 会话池

**Files:**
- Create: `playground/src/lsp/pool.rs`
- Modify: `playground/src/lsp/mod.rs`

**Interfaces:**
- Consumes: `exec::RaProcess`
- Produces: `pool::Pool`、`pool::Handle`（取用时拿到的 RA 会话）、`pool::Acquire`

池是**纯状态机 + 一个「造新容器」的闭包**，容器生命周期由 Task 3 的抽象承担。

- [ ] **Step 1: 写失败的测试**

四个池不变量，对应 Review Focus 第 3 条：

```rust
#[test]
fn empty_pool_yields_a_new_session() { /* 取用一次 → 造了一个 */ }

#[test]
fn idle_never_drops_below_min() {
    // POOL_MIN = 1,池里有 1 个空闲;取用 → 空闲 0;放回 → 空闲 1,什么都不销毁
}

#[test]
fn idle_above_min_is_reclaimed() {
    // POOL_MIN = 1,池里有 2 个空闲;放回一个 → 空闲数仍 2 > 1 → 销毁 1 个
}

#[test]
fn dead_session_is_not_returned_to_the_pool() {
    // Review Focus 第 2 条:ping 失败的 RA 释放时不回池,直接销毁
}
```

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground lsp::pool
```

- [ ] **Step 3: 实现**

`Pool` 持有 `Mutex<VecDeque<Entry>>`（空闲）+ `AtomicUsize`（在用数）+ `Notify`。
`acquire()` 先 pop 空闲（pop 前做 ping 存活性检查，失败就丢弃该 entry 继续 pop，
并保证**至多尝试 `空闲数` 次**，避免全坏时死循环）；空则调 `new_session()` 闭包现造。
`release(entry, alive: bool)`：`alive` 才回池，然后按 `空闲数 > POOL_MIN` 销毁超出部分。

- [ ] **Step 4: 跑测试确认它过**

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/lsp/mod.rs playground/src/lsp/pool.rs && git commit -m "feat(play): RA 容器池(下限 1,超出才回收)

空闲数低于 POOL_MIN 时不回收;超过才 docker rm -f。取用时先 ping 查存活性,
失败的那个不交给用户也不回池 —— 热 RA 卡死是唯一会把坏管道递出去的路径。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: LSP 会话

**Files:**
- Create: `playground/src/lsp/session.rs`
- Modify: `playground/src/lsp/mod.rs`

**Interfaces:**
- Consumes: `exec::RaProcess` + `frame`
- Produces: `session::Session`，方法 `initialize` / `did_open` / `did_change` / `request_completion`，
  以及 `enum session::Event { Diagnostic(..), Tokens(Vec<u32>), Completed { request_id, items } }`

- [ ] **Step 1: 写失败的测试**

用假 `RaCommand` 喂预设帧，断言会话发出的 LSP 消息序列：

```rust
#[tokio::test]
async fn initialize_then_did_open_then_did_change() { /* 消息顺序与 JSON 内容 */ }

#[tokio::test]
async fn did_change_sends_full_text_not_range() {
    // Review Focus 第 1 条的骨架:didChange 不带 range
    assert!(!written.contains("\"range\""));
    assert!(written.contains("\"text\""));
}

#[tokio::test]
async fn publish_diagnostics_becomes_an_event() { /* 喂一帧 publishDiagnostics */ }

#[tokio::test]
async fn semantic_tokens_pass_through_untouched() {
    // 服务端不拆 delta:Event::Tokens 拿到的就是 RA 给的那串
}
```

- [ ] **Step 2: 跑测试确认它失败**

- [ ] **Step 3: 实现**

`initialize` 的 `rootUri` 写死容器的 `/opt/ra/ws`，`capabilities.textDocument.semanticTokens`
必须声明（否则 RA 不给 `semanticTokensProvider`，见设计稿）。

- [ ] **Step 4: 跑测试确认它过**

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/lsp/mod.rs playground/src/lsp/session.rs && git commit -m "feat(play): LSP 会话(initialize/didOpen/didChange/诊断/token)

initialize 必须声明 capabilities.textDocument.semanticTokens,否则 RA 不给
semanticTokensProvider,高亮会是一片空白。

didChange 用全文替换不带 range:单文件、1 MiB 上限,整传比增量省事且不会算错
偏移。semanticTokens 的 delta 数组原样透传,拆分是前端的活。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: 协议消息

**Files:**
- Modify: `playground/src/protocol.rs`

**Interfaces:**
- Consumes: 无。
- Produces: `ClientMsg::{LspOpen, LspChange}`、`ServerMsg::{LspReady, LspTokens, LspDiagnostic, LspCompletion}`、`Hello::lsp_supported`、`PROTOCOL_VERSION = 2`。

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn lsp_open_roundtrips() {
    let m = ClientMsg::LspOpen {};
    let j = serde_json::to_string(&m).unwrap();
    assert_eq!(j, r#"{"type":"lsp_open"}"#);
    assert_eq!(serde_json::from_str::<ClientMsg>(&j).unwrap(), m);
}

#[test]
fn lsp_tokens_is_a_raw_u32_array() {
    let m = ServerMsg::LspTokens { data: vec![0, 0, 3, 6, 0] };
    let j = serde_json::to_string(&m).unwrap();
    assert!(j.contains(r#""data":[0,0,3,6,0]"#), "{j}");
}

#[test]
fn protocol_version_is_2() {
    assert_eq!(PROTOCOL_VERSION, 2);
}
```

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground protocol
```

- [ ] **Step 3: 实现**

按设计稿「协议扩展」一节加变体。`Hello` 加 `lsp_supported: bool`。改 `PROTOCOL_VERSION`
到 2 —— 顺手把 `server/ws.rs` 里比较版本的地方（若已有）一并对齐。

- [ ] **Step 4: 跑测试确认它过 + 整个 crate 仍能编**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/protocol.rs playground/src/server/ws.rs && git commit -m "feat(play): 协议加 LSP 分支,版本抬到 2

protocol.rs 的文件头注释本来就预留了 {\\"type\\":\\"lsp\\"} 双向转发。Hello 带
lsp_supported,让旧前端能优雅降级回今天的行为。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: 配置项

**Files:**
- Modify: `playground/src/config.rs`

**Interfaces:**
- Consumes: 无。
- Produces: `Config` 上 5 个新字段 + 解析。

| 变量 | 类型 | 默认 |
|---|---|---|
| `PLAYGROUND_LSP_DISABLED` | `bool` | `false` |
| `PLAYGROUND_LSP_POOL_MIN` | `usize` | `1` |
| `PLAYGROUND_LSP_IDLE_TTL` | `Duration` | `90s` |
| `PLAYGROUND_LSP_BOOT_TIMEOUT` | `Duration` | `30s` |
| `PLAYGROUND_LSP_MEMORY` | `Option<String>` | `Some("2g")` |

- [ ] **Step 1: 写失败的测试**

照 `config.rs` 既有 `mod tests` 的路子：

```rust
#[test]
fn lsp_defaults() {
    let c = resolve(&Config::default(), &|_| None).unwrap();
    assert_eq!(c.lsp_pool_min, 1);
    assert_eq!(c.lsp_idle_ttl, Duration::from_secs(90));
}

#[test]
fn lsp_zero_pool_min_is_allowed() {
    // 0 = 不预留待命,每次现造(省 839 MB,代价是每次会话等 6-11s)
    let c = resolve(&Config::default(), &|k| (k == "PLAYGROUND_LSP_POOL_MIN").then(|| "0".into())).unwrap();
    assert_eq!(c.lsp_pool_min, 0);
}

#[test]
fn lsp_disabled_flag() {
    let c = resolve(&Config::default(), &|k| (k == "PLAYGROUND_LSP_DISABLED").then(|| "1".into())).unwrap();
    assert!(c.lsp_disabled);
}
```

签名与 `config.rs:241` 的 `getter` 辅助对齐。

- [ ] **Step 2: 跑测试确认它失败**

- [ ] **Step 3: 实现**

用既有的 `raw` / `num_var` / `bool_var` / `str_var`。**默认值只写在 `Config` 的 `Default` 一处**，
其余引用字段（照 `config.rs:1` 的文件头要求）。

- [ ] **Step 4: 跑测试确认它过**

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/config.rs && git commit -m "feat(play): LSP 的 5 个环境变量

POOL_MIN 默认 1(IDLE_TTL 90s 是内存策略不是性能策略:热池下省 839 MB 换 6-11s
重建)。0 = 不预留待命。DISABLED 一键关掉整条链路。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: 镜像加 RA stage

**Files:**
- Modify: `playground/src/image_builder.rs`

**Interfaces:**
- Consumes: `Config::rustup_dist_server` / `cargo_mirror`（已有）
- Produces: 第二个镜像 tag `<image>-ra`，里面 `/opt/ra/ws` 是裁过的三成员 workspace。

- [ ] **Step 1: 写失败的测试**

`image_builder.rs` 若有构造 Dockerfile 文本的纯函数，就断言它包含 `rustup component add rust-analyzer`
与 `COPY lib-internal ./lib-internal`；若无，先把 Dockerfile 生成抽成一个纯函数再测（这一步是测试的前提，
TDD 里「为了可测而做的最小重构」是允许的）。

- [ ] **Step 2: 跑测试确认它失败**

- [ ] **Step 3: 实现**

Dockerfile 末尾追加：

```dockerfile
FROM rust2genshin-play:base AS ra
RUN rustup component add rust-analyzer
WORKDIR /opt/ra/ws
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY demo ./demo
COPY lib ./lib
COPY lib-internal ./lib-internal
RUN sed -i -e '/^members/,/^]/c\members = ["demo", "lib", "lib-internal"]' \
          -e '/^default-members/,/^]/c\default-members = ["demo", "lib", "lib-internal"]' Cargo.toml
RUN cargo metadata --format-version 1 --offline > /dev/null
```

最后那行 `cargo metadata` 自检很重要 —— 裁错了会在**烘焙时**炸，而不是在用户第一次编辑时。
（手写 `Cargo.toml` 漏掉 `workspace.package.authors` 会让 `cargo metadata` 静默失败，
这是实测踩过的。）

- [ ] **Step 4: 跑测试确认它过**

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/image_builder.rs && git commit -m "feat(play): 镜像加 ra stage(带 rust-analyzer 的裁剪 workspace)

core 不进 RA 镜像:它是 rustc_private 后端,会拖整个 sysroot 进分析(84s vs
11s),而 demo 的依赖链上本来就没有它。

烘焙时跑一次 cargo metadata 自检 —— 裁错了在烘焙时炸,而不是用户第一次编辑时。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 9: 服务端接线

**Files:**
- Modify: `playground/src/server/mod.rs`（`AppState`）
- Modify: `playground/src/server/ws.rs`
- Modify: `playground/src/main.rs`

**Interfaces:**
- Consumes: Task 4/5/6/7 的全部产物
- Produces: 端到端的 `LspOpen` → `LspReady` / `LspTokens` / `LspDiagnostic` / `LspCompletion`

- [ ] **Step 1: 写失败的测试**

`testing.rs` 里那套集成测试加一条：

```rust
// 真起一个容器,走完 LspOpen → LspTokens
```

以及一条纯逻辑的：断开连接后 RA 进程被杀掉（Review Focus 第 4 条）。

- [ ] **Step 2: 跑测试确认它失败**

- [ ] **Step 3: 实现**

`main.rs::run()` 里：`PLAYGROUND_LSP_DISABLED` 为真时 `AppState::lsp = None`，
`ws.rs` 见到 `LspOpen` 回一条 `Error` 说明功能已关（前端据此不挂 LSP 客户端）。
非空时 `tokio::spawn` 两个后台任务：一个补池（空闲就绪数低于 `POOL_MIN` 时造），
一个回收（空闲超 `IDLE_TTL` 且超出下限时 `docker rm -f`）。

**连接断开时的清理**要在 `ws.rs` 的连接处理函数末尾挂一个 guard，仿 `runner.rs:129-185`
的 `ContainerGuard`：`Drop` 里释放池里的 RA 会话。

- [ ] **Step 4: 跑测试确认它过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/server/mod.rs playground/src/server/ws.rs playground/src/main.rs playground/src/testing.rs && git commit -m "feat(play): LSP 接线 —— AppState、ws 分发、后台补池与回收

连接断开时必须释放池里的 RA 会话,否则 839 MB 挂在容器里不放。清理靠 guard
的 Drop(仿 runner.rs 的 ContainerGuard),不依赖任何一条手工路径。

DISABLED 时 AppState::lsp 为 None,ws 回 Error 让前端不挂 LSP 客户端。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 10: 前端 CodeMirror + RA 扩展

**Files:**
- Create: `docs/typst/play/lsp-client.js`
- Modify: `docs/typst/play/script.js`（改成 ESM，装编辑器）
- Modify: `docs/typst/play/index.typ`（编辑框换挂载点 + module script）
- Modify: `docs/typst/play/style.css`（编辑框样式）

**Interfaces:**
- Consumes: Task 6 的 WebSocket 协议
- Produces: 用户可见的编辑器

- [ ] **Step 1: 写下手工验收清单**（前端无测试基建，用清单代替自动测试）

```
[ ] 打开摆弄页,编辑框出现,没有横向滚动条(320/400/800px 三个宽度)
[ ] RA 就绪后编辑框有高亮,且**与编译产物一致**:函数名、宏、生命周期标识都染色
[ ] 输入一个不存在的函数名,1-2 秒内出现红色波浪线,消息是 RA 的原文
[ ] 删掉那行,波浪线消失
[ ] Review Focus 第 1 条:先写 `fn foo() {}`,编译通过;改名成 `bar` 并等 300ms 后,
    在下一行敲 `fo` —— 补全里**不应**再出现 `foo`
[ ] 敲 `rust2genshin_lib::sc::` 出现补全列表,能选中、能插入
[ ] 主题切换(浅/深)后高亮仍然可读
[ ] 服务设 PLAYGROUND_LSP_DISABLED=1 重启,页面仍可用,编辑框无高亮但能编译
[ ] 关掉标签页 30 秒后,容器内 RA 进程消失
```

- [ ] **Step 2: `lsp-client.js`**

薄桥：`LspClient` 把 `LspTokens` / `LspDiagnostic` / `LspCompletion` 转成
CodeMirror 能用的回调，把 CodeMirror 的编辑事件去抖 300ms 后发 `LspChange`。
去抖 300ms 是 Review Focus 第 1 条的落点 —— 早于此会发出半行代码，晚于此补全体感差。

- [ ] **Step 3: `script.js` 改 ESM，装 CodeMirror**

顶部加：

```js
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "https://esm.sh/@codemirror/view@6";
import { EditorState, StateField, StateEffect } from "https://esm.sh/@codemirror/state@6";
import { CompletionSource, closeBrackets } from "https://esm.sh/@codemirror/autocomplete@6";
```

`@codemirror/lang-rust` **不要** —— 高亮来自 RA。

- [ ] **Step 4: 语义高亮扩展**

RA 的 delta 数组格式：`[Δline, ΔstartChar, length, tokenType, tokenModifiers, ...]`，
相对上一个 token 编码。扩展里做一个 `StateField` 持有原始数组，每次
`LspTokens` 到达时 dispatch 一个 effect 重算 decorations。token type 的名字表从
`initialize` 响应的 `semanticTokensProvider.legend` 取。

**高亮与诊断必须是两个独立的层**（Review Focus 第 5 条）：前者是 `Decoration.mark`，
后者是 `Decoration.mark` + 波浪线 gutter，两者可同时存在于同一 range。

- [ ] **Step 5: `index.typ` / `style.css`**

`<mdui-text-field textarea>` 换成一个空的 `<div id="play-editor" class="play-editor">`。
样式从 `.play-left > mdui-text-field` 改成 `.play-editor`（`flex: 1 1 0; min-height: 0;`，
以及 CodeMirror 主题要能跟着站点的深浅色走）。

script 标签要用 `type="module"`，`import_script` 未必发得对 —— 若不对，
直接用 `html_elem("script", attrs: ("type": "module", "src": base_path + "/play/script.js"))[]`。

- [ ] **Step 6: 手工验收**

挂 `cargo run -p rust2genshin-docs --bin watch`，开 `http://127.0.0.1:3000/play/`
逐条走 Step 1 的清单。**后台服务要在服务端跑**（`/mnt/server/r2g-srv/run-playground.sh`
重建容器），本地浏览器连公网地址。

- [ ] **Step 7: 提交**

```bash
cd C:/projects/rust2genshin && git add docs/typst/play/index.typ docs/typst/play/script.js docs/typst/play/lsp-client.js docs/typst/play/style.css && git commit -m "feat(play): 编辑器换 CodeMirror 6,高亮/诊断/补全全走 RA

不引入 @codemirror/lang-rust 或任何 Rust 解析器 —— 三样能力都是 RA 经 LSP
给的。本地零解析,不存在「高亮和分析结论不一致」。

高亮与诊断是两个独立的 decoration 层,可同时存在于同一 range;高亮要等 RA
就绪才出现,UI 上给一条「分析中…」而不是留纯黑空窗。

编辑去抖 300ms:早于此发半行代码,晚于此补全体感差。

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 11: 文档站更新

**Files:**
- Modify: `docs/typst/play/index.typ`（说明文字）
- Create/Modify: `docs/typst/dev/playground.typ`

- [ ] **Step 1: 写内容**

站点「playground」页补一节：编辑器的三个能力来自容器内的 RA、RA 镜像为什么裁到
三个成员、待命池的内存代价（839 MB/个）、`PLAYGROUND_LSP_*` 配置项表。

按 `docs/typst/lib/lib.typ` 与 `outline.typ` 现有的写法组织，不要新造结构。

- [ ] **Step 2: 起 watch 看渲染**

```
cd C:/projects/rust2genshin && cargo run -p rust2genshin-docs --bin watch
```
查它的输出确认没有排版报错。

- [ ] **Step 3: 提交**

```bash
cd C:/projects/rust2genshin && git add docs/typst/dev/playground.typ docs/typst/play/index.typ && git commit -m "docs(play): 站点补 RA 编辑器的原理与代价

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## 自查

- **Spec 覆盖** —— 设计稿的每一节都落到了某个任务：传输层（Task 2/3）、workspace 裁剪（Task 8）、
  池（Task 4）、生命周期与就绪判据（Task 5/9）、协议（Task 6）、配置（Task 7）、
  前端（Task 10）、镜像（Task 8）、测试（各任务内联 + Task 10 清单）、风险（Review Focus）。
- **占位符** —— 无 TBD / 无「类似 Task N」。Task 3/4/5 的实现步骤给的是结构约束而非逐行代码，
  这是有意的：这三处的形状由前两任务的 trait 决定，写死会立刻过期。它们的**测试**是完整的，
  实现照测试写即可。
- **类型一致** —— `FrameDecoder::push -> Result<Vec<Vec<u8>>, FrameError>` 在 Task 1 定义，
  Task 5 用它；`RaCommand` 在 Task 3 定义，Task 4/5 用它；`Pool::acquire -> Handle` 在 Task 4
  定义，Task 9 用它。
- **Review Focus 六条** —— 1 落在 Task 5 + Task 10 的清单第 4 条；2 落在 Task 4；3 落在 Task 4；
  4 落在 Task 3 的 Drop + Task 9 的 guard + Task 10 清单第 9 条；5 落在 Task 10 Step 4；
  6 落在 Task 6。
