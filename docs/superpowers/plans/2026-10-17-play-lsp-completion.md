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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
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
    fn run_detached_puts_d_before_the_image() {
        // `-d` 必须在 image **之前**。追加到末尾的话它落在 image 之后，成了
        // `sh -c ...` 的 `$0`（不是 docker 的 flag），容器反而以前台跑 cargo
        // —— 而待命容器要跑的恰恰是 sleep。
        let a = run_detached_args(&ra_spec());
        let d = a.iter().position(|s| s == "-d").expect("要有 -d");
        let img = a.iter().position(|s| s == "img:tag").expect("要有 image");
        assert!(d < img, "-d 在 {} 位，image 在 {} 位", d, img);
    }

    #[test]
    fn run_detached_drops_the_compile_command() {
        // image 之后只剩 sleep：编译任务那条 `sh -c cargo build ...` 必须整个
        // 去掉，否则待命容器会真去编译一遍。
        let a = run_detached_args(&ra_spec());
        let img = a.iter().position(|s| s == "img:tag").unwrap();
        assert_eq!(&a[img + 1..], &["sleep".to_string()]);
        assert!(!a.iter().any(|s| s.contains("cargo build")));
    }

    #[test]
    fn run_detached_has_no_attach_flags_and_no_mounts() {
        // -i 会占住管道（RA 由 exec 另起），-t 是伪终端。待命容器自己不需要
        // demo/src 与 /out —— 它的 workspace 在镜像里，源码由 exec 写进去。
        let a = run_detached_args(&ra_spec());
        assert!(!a.iter().any(|s| s == "-i" || s == "-t"));
        assert!(!a.iter().any(|s| s.contains("demo/src")));
        assert!(!a.iter().any(|s| s.contains("/out")));
    }

    /// 共用的 spec 工厂 —— 三个 run_detached 测试都只关心 argv 形状。
    /// 字段以 `docker.rs:98-124` 的 `RunSpec` 实际定义为准（`src_dir` /
    /// `out_dir` 是 `PathBuf`，可能要 `PathBuf::from(...)`）。
    fn ra_spec() -> RunSpec {
        RunSpec {
            name: "r2g-ra-1".into(),
            image: "img:tag".into(),
            src_dir: "/src".into(),
            out_dir: "/out".into(),
            workdir: "/opt/ra/ws".into(),
            cpus: Some(2),
            memory: Some("2g".into()),
            pids_limit: 128,
            user: "1001".into(),
            source_file: "demo/src/lib.rs".into(),
        }
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

/// RA 容器里的裁剪 workspace 根。镜像的 ra stage 就装在这儿。
pub const RA_WORKDIR: &str = "/opt/ra/ws";

/// 待命 RA 容器：`-d` 后台、不 attach、末尾跑 `sleep`。
///
/// **不能靠 `run_args` 拼。** `run_args` 的结尾是
/// `image, sh, -c, container_command`，把 `-d` 追加到末尾的话：`-d` 落在
/// image 之后，就成了 `sh` 的 `$0`（不是 docker 的 flag），容器反而**前台跑
/// cargo build** —— 而待命容器要跑的恰恰是 sleep。
///
/// 所以这里从 `limit_args` 起手自己拼：
///   - `--workdir` 给 `/opt/ra/ws`（RA 镜像里的裁剪 workspace），不是 demo 的；
///   - **不挂** `demo/src` 与 `/out` —— RA 的源码由 `exec` 写进容器自己的
///     文件系统，挂宿主目录既没必要又要多一份目录簿记；
///   - `--rm` 保留：待命容器只在被 kill 时退出，退出即清，不留残容器。
///
/// 顺带要求：把 `run_args` 里「资源限额」那几段（`--network` / `--pids-limit`
/// / `--user` / `--tmpfs` / `--memory` / `--cpus` / `--cap-drop` /
/// `--security-opt`）抽成一个私有 `fn limit_args(spec) -> Vec<String>`，两个
/// 函数共用。限额必须一致 —— 否则 RA 容器拿不到和编译任务同样的保护，而它
/// 常驻 839 MB，比编译容器危险得多。`run_args` 的行为一个字都不能变（它有
/// 既有的测试在盯着）。
pub fn run_detached_args(spec: &RunSpec) -> Vec<String> {
    let mut a = vec!["run".into()];
    a.extend(limit_args(spec));
    a.push("--rm".into());
    a.push("--name".into());
    a.push(spec.name.clone());
    a.push("--workdir".into());
    a.push(RA_WORKDIR.to_string());
    a.push("-d".into());
    a.push(spec.image.clone());
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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
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
        // 那条 JSON 是 58 字节 —— 手数容易错,别写死一个猜的数,
        // 否则失败会被误读成「拆帧算错了字节」。
        assert!(w[0].starts_with("Content-Length: 58\r\n\r\n"), "实际:{}", &w[0][..40]);
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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: LSP 会话

**Files:**
- Create: `playground/src/lsp/session.rs`
- Modify: `playground/src/lsp/mod.rs`

**Interfaces:**
- Consumes: `exec::RaProcess` + `frame`
- Produces: `session::Session`，方法 `initialize` / `did_open` / `did_change` /
  `request_completion(request_id, line, column)`，
  以及 `enum session::Event { Diagnostics(Vec<Diagnostic>), Tokens(Vec<u32>), Completed { request_id, items } }`。

`Event::Diagnostics` 是**按通知批量**的，且**空 vec 也要发** —— LSP 用空数组表示「该文件的诊断清空」，
不发的话用户修好错误后红线永远不消失。Session 内部按 `DOC_URI` 过滤，`lib` / `lib-internal` /
`Cargo.toml` 的诊断不往上抛（单文件编辑器，画到别的行上只会是错的）。

`request_completion` 的 `request_id` 来自前端，服务端**原样用作 JSON-RPC 的 `id`**，
响应回来时再从 `Event::Completed` 里原样带回。不要在这一层另发一个内部 id 再映射：
`initialize` 在 `LspReady` 之前就已经 await 完，之后服务端不持有任何未完成的请求，
客户端 id 撞不上。

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
async fn completion_request_uses_the_callers_id() {
    // 前端给 7,写进管道的 JSON-RPC id 就得是 7 —— 服务端不另发内部 id
    assert!(written.contains(r#""id":7"#));
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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: 协议消息

**Files:**
- Modify: `playground/src/protocol.rs`

**Interfaces:**
- Consumes: 无。
- Produces: `ClientMsg::{LspOpen, LspChange, LspCompletion}`、`ServerMsg::{LspReady, LspTokens, LspDiagnostic, LspCompletion}`、`Hello::lsp_supported`、`PROTOCOL_VERSION = 2`。

**为什么 `ClientMsg` 必须有 `LspCompletion`**：LSP 的补全是请求-应答，而 `ServerMsg` 是单向广播流，
前面只有 `LspOpen` / `LspChange` 两条客户端消息，服务端没有任何入口知道「用户要补全了」——
`ServerMsg::LspCompletion` 会永远发不出来。`request_id` 是前端自己产生的序号，服务端原样
用作 JSON-RPC 的 `id` 并原样回显；`LspOpen` → `LspReady` 之后、补全之前服务端不持有任何
未完成的 JSON-RPC 请求（`initialize` 已经 await 完），所以客户端的 id 不会和服务端的撞。

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
fn lsp_completion_reports_the_cursor() {
    let m = ClientMsg::LspCompletion { request_id: 7, line: 3, column: 12 };
    let j = serde_json::to_string(&m).unwrap();
    assert_eq!(j, r#"{"type":"lsp_completion","request_id":7,"line":3,"column":12}"#);
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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
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

照 `config.rs` 既有 `mod tests` 的路子（`getter` + `resolve`；**`Config` 没有 `Default`，
`resolve` 的第一个参数是环境查表函数，第二个是 `manifest_dir`**）：

```rust
#[test]
fn lsp_defaults() {
    let g = getter(&[]);
    let c = resolve(&g, Path::new("/repo/playground")).unwrap();
    assert!(!c.lsp_disabled);
    assert_eq!(c.lsp_pool_min, 1);
    assert_eq!(c.lsp_idle_ttl, Duration::from_secs(90));
    assert_eq!(c.lsp_boot_timeout, Duration::from_secs(30));
    assert_eq!(c.lsp_memory.as_deref(), Some("2g"));
}

#[test]
fn lsp_zero_pool_min_is_allowed() {
    // 0 = 不预留待命,每次现造(省 839 MB,代价是每次会话等 6-11s)
    let g = getter(&[("PLAYGROUND_LSP_POOL_MIN", "0")]);
    let c = resolve(&g, Path::new("/repo/playground")).unwrap();
    assert_eq!(c.lsp_pool_min, 0);
}

#[test]
fn lsp_disabled_flag() {
    let g = getter(&[("PLAYGROUND_LSP_DISABLED", "1")]);
    let c = resolve(&g, Path::new("/repo/playground")).unwrap();
    assert!(c.lsp_disabled);
}
```

- [ ] **Step 2: 跑测试确认它失败**

- [ ] **Step 3: 实现**

用既有的 `raw` / `num_var` / `bool_var`。**默认值只写在 `resolve` 这一处**（`config.rs:1`
的文件头要求），`Config` 本身没有 `Default`。

`PLAYGROUND_LSP_MEMORY` 需要 `memory_var` 的语义但默认值不同：把 `memory_var` 改成
`memory_var(key, get, default: &str)`，既有调用传 `"1536m"`，LSP 的传 `"2g"`。
`none` 仍然表示不下 `--memory`。

- [ ] **Step 4: 跑测试确认它过**

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/config.rs && git commit -m "feat(play): LSP 的 5 个环境变量

POOL_MIN 默认 1(IDLE_TTL 90s 是内存策略不是性能策略:热池下省 839 MB 换 6-11s
重建)。0 = 不预留待命。DISABLED 一键关掉整条链路。

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 8: 镜像加 RA stage

**Files:**
- Modify: `playground/Dockerfile`
- Modify: `playground/src/image_builder.rs`（只加测试断言，**不改构建逻辑**）

**Interfaces:**
- Consumes: `Config::rustup_dist_server` / `cargo_mirror`（已有）
- Produces: **同一个镜像里**多出 `/opt/ra/ws`（裁过的三成员 workspace）与 `rust-analyzer` 组件。

**为什么不做第二个镜像 tag**：计划最初写的是 `FROM <play-image> AS ra` 再打一个 `<image>-ra` tag。
放弃它的理由有三条，每条都能落到代码：

1. Dockerfile 现在最后一段是未命名的默认 stage（`playground/Dockerfile` 末尾是 `CMD`）。在后面追加
   `FROM … AS ra` 会把**默认构建目标变成 `ra`**，任务容器的镜像就没了。
2. 多一个 tag 就多一倍镜像数，而 `gc_loop::sweep_images` 按 `keep_images`（默认 2）计数 —— 不一起改
   就会把 play 镜像挤掉。
3. 收益只是「RA 镜像里没有 core/build-demo」的观感。RA 的分析范围由 `/opt/ra/ws/Cargo.toml` 决定，
   不由磁盘上还有别的目录决定；实测 11s 的裁剪效果照旧。

代价：play 镜像多一个 rust-analyzer 组件（约百来 MB）与 `/opt/ra/ws` 那点源码。相对已有的工具链 + `target/`
是零头。

- [ ] **Step 1: 写失败的测试**

`image_builder.rs` 的 `mod tests` 里已经有一个直接读 Dockerfile 文本的测试
（`dockerfile_puts_cargo_on_the_task_containers_path`）。照它的写法加一条：

```rust
#[test]
fn dockerfile_bakes_the_trimmed_ra_workspace() {
    let dockerfile = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Dockerfile"),
    )
    .expect("读 playground/Dockerfile");
    assert!(dockerfile.contains("rustup component add rust-analyzer"), "{dockerfile}");
    assert!(dockerfile.contains("/opt/ra/ws"), "{dockerfile}");
    // 裁过的成员表必须只留三个 —— core 进来会把整个 sysroot 拖进分析
    assert!(dockerfile.contains(r#""demo", "lib", "lib-internal""#), "{dockerfile}");
    assert!(!dockerfile.contains(r#""core""#), "{dockerfile}");
    // 烘焙时自检:裁错了在烘焙时炸,而不是用户第一次编辑时
    assert!(dockerfile.contains("cargo metadata"), "{dockerfile}");
    // 两条顺序/结构不变量 —— 文字断言拦不住它们,但改错了行为全变:
    // RA 的 RUN 必须在 USER builder 之后,否则 /opt/ra/ws 归 root,--user 1001 写 target/ 会 EACCES。
    // 必须匹配**指令行**:Dockerfile 的注释里也出现 "USER builder",直接 find 会命中注释而空转。
    let user_line = dockerfile
        .lines()
        .position(|l| l.trim() == "USER builder")
        .expect("Dockerfile 里必须有 USER builder 指令");
    let ra_run_line = dockerfile
        .lines()
        .position(|l| l.trim_start().starts_with("RUN ") && l.contains("/opt/ra/ws"))
        .expect("必须有建 /opt/ra/ws 的 RUN");
    assert!(user_line < ra_run_line, "RA workspace 必须在 USER builder 之后创建");
    // 末尾不许再出现 FROM:追加 stage 会让默认构建目标变成它,任务容器的镜像就没了。
    assert!(
        !dockerfile.lines().skip(ra_run_line).any(|l| l.trim_start().starts_with("FROM ")),
        "RA 相关指令之后不许再开 stage"
    );
}
```

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground dockerfile_bakes
```

- [ ] **Step 3: 实现**

先把**已有的**建用户那条 RUN（`Dockerfile:36-38`）补上 `/opt/ra` 的预建：

```dockerfile
RUN useradd -u 1001 -m -s /bin/bash builder \
 && mkdir -p /opt/rust2genshin /opt/ra \
 && chown builder:builder /opt/rust2genshin /opt/ra
```

> **必须由 root 建 `/opt/ra`。** `/opt` 在 `debian:bookworm-slim` 里是
> `root:root 755`，`USER builder`（uid 1001）在其中 `mkdir` 会
> `Permission denied`，镜像直接构建失败。`/opt/ra` 一旦归 builder，下面
> `mkdir -p /opt/ra/ws` 建出来的 `ws` 天然也是 1001 的，RA 容器
> `--user 1001` 才能写 `target/`。

然后在 `playground/Dockerfile` 的 `RUN . "$HOME/.cargo/env" && cargo run -p build-demo --bin beyond-server`
**之后**、`CMD` 之前追加：

```dockerfile
# rust-analyzer 组件。RA 容器与任务容器用同一个镜像 —— 见计划里 Task 8 的说明,
# 不做第二个 tag(会翻倍镜像数,撞上 gc 的 keep_images)。
RUN . "$HOME/.cargo/env" && rustup component add rust-analyzer

# RA 的分析工作区。**只放 demo / lib / lib-internal 三个成员**:core 是
# rustc_private 后端,RA 为解析它要把整个编译器 sysroot 的元数据拉进来,
# 冷启动从 11s 变成 84s,而 demo 的依赖链上根本没有 core。
#
# 手写一份精简 Cargo.toml 会漏掉 workspace.package.* 的继承项,cargo metadata
# 会静默失败 —— 所以是复制根清单再改 members,不是新写一份。
RUN mkdir -p /opt/ra/ws \
 && cp rust-toolchain.toml Cargo.toml Cargo.lock /opt/ra/ws/ \
 && cp -r demo lib lib-internal /opt/ra/ws/ \
 && sed -i -e '/^members/,/^]/c\members = ["demo", "lib", "lib-internal"]' \
          -e '/^default-members/,/^]/c\default-members = ["demo", "lib", "lib-internal"]' \
          /opt/ra/ws/Cargo.toml \
 && cd /opt/ra/ws \
 && . "$HOME/.cargo/env" \
 && cargo metadata --format-version 1 --offline > /dev/null
```

`/opt/ra/ws` 由 `builder`（uid 1001）创建，与 `docker.rs` 的 `RA_WORKDIR` 以及 RA 容器的
`--user 1001` 对得上 —— 否则 RA 写 `target/` 时 EACCES，表现为「一直分析中」。

- [ ] **Step 4: 跑测试确认它过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground dockerfile_bakes
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/Dockerfile playground/src/image_builder.rs && git commit -m "feat(play): 镜像里烘 rust-analyzer 与裁剪版 workspace

core 不进 RA 的分析范围:它是 rustc_private 后端,会拖整个 sysroot 进分析
(84s vs 11s),而 demo 的依赖链上本来就没有它。

不做第二个镜像 tag:会翻倍镜像数,撞上 gc 的 keep_images。

烘焙时跑一次 cargo metadata 自检 —— 裁错了在烘焙时炸,而不是用户第一次编辑时。

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 9: RA 服务运行时（真实 exec + 可复用会话）

> **预检改写了这一节。** 原稿只有一句「接池」，但代码里缺三块真的会炸的东西：
>
> 1. **没有真实的 `RaCommand` 实现**。`RaCommand` 只在 `exec.rs` / `pool.rs` / `session.rs`
>    的 `#[cfg(test)]` 里有假实现；`RaProcess::start` 需要 `Arc<dyn RaCommand>`，
>    服务端没有任何东西能交出真的。
> 2. **`Session::events()` 只能调一次**（`session.rs:322-330` 的 `take()`）。池的语义是
>    一个会话跨多条 WebSocket 连接复用，第一条连接断开时 receiver 被 drop，第二条连接
>    再取就是 `panic`（或事件无处可去）。
> 3. **池的存活性检查会和 reader 抢同一根管道**。`Pool::acquire` 调
>    `handle.process.ping()`（`pool.rs:138-142`），而 `Session` 的后台 reader 在同一个
>    `RaProcess` 上循环 `recv`。`ping` 的响应可能被 reader 吃掉 → 健康会话被误判死亡；
>    reader 也可能被 ping 抢走一帧 token/诊断。
>
> 因此本节先做「可复用的 RA 运行时」，Task 9b 才接 `AppState` / `ws.rs`。

**Files:**
- Create: `playground/src/lsp/docker_exec.rs`
- Create: `playground/src/lsp/service.rs`
- Modify: `playground/src/lsp/mod.rs`
- Modify: `playground/src/lsp/session.rs`
- Modify: `playground/src/lsp/pool.rs`
- Modify: `playground/src/docker.rs`

**Interfaces:**
- Consumes: Task 3 的 `RaProcess`、Task 4 的 `Pool`、Task 5 的 `Session`、Task 7 的配置项
- Produces（Task 9b 要用）：
  - `LspService::new(cfg: Arc<Config>, baker: Arc<Baker>) -> Arc<LspService>`
  - `LspService::acquire(&self) -> Result<Acquire, PoolError>`
  - `LspService::write_source(&self, container: &str, source: &str) -> Result<(), String>`
  - `LspService::spawn_background(self: &Arc<Self>)`（补池 + 回收）
  - `Acquire::session(&self) -> &Arc<Session>`、`Session::subscribe() -> broadcast::Receiver<Event>`
  - `Handle::new(process, destroy)` + `Handle::with_liveness(...)`

- [ ] **Step 1: 写失败的测试**

三组，都不碰真 docker：

**(a) `Session::ping` 走 pending 表，不碰裸管道**（`session.rs` 的测试里）：

```rust
#[tokio::test]
async fn session_ping_goes_through_the_pending_map() {
    let (fake, session) = setup().await;
    let feeder = tokio::spawn({
        let fake = Arc::clone(&fake);
        async move {
            // 等 ping 请求真的写出去,再按它的 id 回一条 **error** 响应:
            // 未知方法 RA 就是这么回的 —— 有响应即证明活着。
            let id = fake.wait_until_written("\"method\":\"r2g/ping\"").await;
            let resp = serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "method not found" }
            });
            fake.sender().send(frame::encode(&resp.to_string())).unwrap();
        }
    });
    session.ping(Duration::from_secs(1)).await.expect("error 响应也算活着");
    feeder.await.unwrap();

    // 沉默必须超时,不能 hang。
    let (fake2, session2) = setup().await;
    assert!(session2.ping(Duration::from_millis(50)).await.is_err());
    drop(fake2);
}
```

`wait_until_written` 现在只返回 `()`，把它改成返回**写出去的那条请求的 `id`**
（`Option<u64>`）：在攒下来的字节里 `FrameDecoder` 解帧、从后往前找包含 needle 的那条 JSON。
这是本任务唯一对测试夹具的改动。

**(b) 池的存活性检查可替换，且 `acquire` 能报失败**（`pool.rs` 的测试里）：

```rust
#[tokio::test]
async fn acquire_uses_the_handles_liveness_probe() {
    let state = FakeState::new();
    let pool = new_pool(Arc::clone(&state), true);
    let h = make_handle(&state, true).await;             // 裸管道 ping 会失败(没人回)
    let probed = Arc::new(AtomicUsize::new(0));
    let p = Arc::clone(&probed);
    let h = h.with_liveness(move || {
        let p = Arc::clone(&p);
        Box::pin(async move { p.fetch_add(1, Ordering::SeqCst); true })
    });
    pool.idle.lock().await.push_back(h);

    let acq = pool.acquire().await.expect("liveness 说活就该拿到");
    assert_eq!(probed.load(Ordering::SeqCst), 1, "必须用 handle 自己的探针");
    assert_eq!(state.created(), 1, "不该因为裸管道 ping 不通就重造");
    acq.release(true).await;
}

#[tokio::test]
async fn acquire_reports_a_failed_creation() {
    // new_session 返回 None(镜像没就绪 / docker 起不来)时,acquire 必须
    // 报错而不是交出一个假的会话。
    let pool = Pool::new(|| async { None });
    assert!(pool.acquire().await.is_err());
}
```

**(c) 回收只动空闲且超龄的，且保留下限**（`pool.rs`）：

```rust
#[tokio::test]
async fn sweep_idle_destroys_only_expired_entries_above_the_minimum() {
    let state = FakeState::new();
    let pool = new_pool(Arc::clone(&state), true);
    pool.idle.lock().await.push_back(make_handle(&state, true).await);
    pool.idle.lock().await.push_back(make_handle(&state, true).await);
    assert_eq!(state.created(), 2);

    // TTL = 0 也一个都不删:空闲数 == POOL_MIN 时没有「超出」的部分。
    assert_eq!(pool.sweep_idle(Duration::ZERO).await, 0);
    assert_eq!(pool.idle.lock().await.len(), 2);

    // 给两条都标成很久以前,再放一条新的 —— 超出 POOL_MIN=1 的那条才被删。
    {
        let mut idle = pool.idle.lock().await;
        for h in idle.iter_mut() { h.idle_since = Instant::now() - Duration::from_secs(3600); }
    }
    let fresh = make_handle(&state, true).await;
    {
        let mut idle = pool.idle.lock().await;
        idle.push_back(fresh);
    }
    assert_eq!(pool.sweep_idle(Duration::from_secs(90)).await, 2);
    assert_eq!(pool.idle.lock().await.len(), 1, "至少留 POOL_MIN 个");
    assert_eq!(state.destroyed(), 2);
}
```

- [ ] **Step 2: 跑测试确认它们失败**

```
cd C:/projects/r2genshin && cargo test -p rust2genshin-playground lsp::
```

（在仓库根跑：`cd C:/projects/rust2genshin`。）

- [ ] **Step 3: 实现**

**3.1 `session.rs`：事件改成 broadcast，加 `ping`。**

- `events_rx: Mutex<Option<UnboundedReceiver<Event>>>` 换成
  `events_tx: tokio::sync::broadcast::Sender<Event>`（容量 64）。
  `Session::new` 里 `let (events_tx, _) = broadcast::channel(64);`，reader 拿到
  `Sender` 的 clone，`dispatch` / `emit_diagnostics` 把 `tx.send(e)` 改成
  `let _ = tx.send(e)`（无接收者时是正常的：会话在池里待命）。
- `pub fn subscribe(&self) -> broadcast::Receiver<Event> { self.events_tx.subscribe() }`，
  删掉 `events()`。所有测试里的 `session.events()` 改成 `session.subscribe()`。
- `broadcast::Receiver::recv` 的错误是 `RecvError::Lagged`（消费者慢了）与
  `Closed`（reader 退出）。测试辅助 `next_event` 遇到 `Lagged` 直接 panic 并打印
  「测试里不该落后」。
- `Session::ping`：

  ```rust
  /// 存活性检查。**必须走 pending 表**：直接读 `RaProcess` 会和后台 reader
  /// 抢同一根管道,把 token/诊断帧偷走,或让健康会话被误判死亡。
  /// RA 对未知方法回 `error` 响应 —— 有响应就是活着。
  pub async fn ping(&self, timeout: Duration) -> Result<(), LspError> {
      let id = self.next_id();
      let (tx, rx) = oneshot::channel();
      self.pending.lock().expect("Mutex 不该中毒:持锁段不 panic")
          .insert(id, Pending::Response(tx));
      let req = json!({
          "jsonrpc": "2.0", "id": id, "method": "r2g/ping", "params": Value::Null
      });
      if let Err(e) = self.send_json(&req).await {
          self.unregister(id);
          return Err(e);
      }
      match tokio::time::timeout(timeout, rx).await {
          Ok(Ok(_)) => Ok(()),
          Ok(Err(_)) => Err(LspError::Closed),
          Err(_) => { self.unregister(id); Err(LspError::Timeout) }
      }
  }
  ```

**3.2 `pool.rs`：Handle 带可替换的存活性探针、会话、idle 时间戳；`acquire` 返回 Result。**

```rust
type Liveness =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync + 'static>;

pub struct Handle {
    process: Arc<RaProcess>,
    session: Arc<Session>,
    liveness: Liveness,
    pub(crate) idle_since: Instant,
    destroy: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl Handle {
    /// 默认探针是 ping 裸管道 —— 只给 pool 的单元测试用(那条管道上没有
    /// reader 竞争)。**真实会话必须 `with_liveness` 换成 `Session::ping`。**
    pub fn new(
        process: Arc<RaProcess>,
        session: Arc<Session>,
        destroy: Box<dyn FnOnce() + Send + 'static>,
    ) -> Self {
        let p = Arc::clone(&process);
        Self {
            process, session,
            liveness: Box::new(move || {
                let p = Arc::clone(&p);
                Box::pin(async move { p.ping(Duration::from_secs(1)).await.is_ok() })
            }),
            idle_since: Instant::now(),
            destroy: Some(destroy),
        }
    }

    pub fn with_liveness(mut self, f: impl Fn() -> Pin<Box<dyn Future<Output = bool> + Send>>
        + Send + Sync + 'static) -> Self { self.liveness = Box::new(f); self }
    pub fn session(&self) -> &Arc<Session> { &self.session }
    pub fn process(&self) -> &Arc<RaProcess> { &self.process }
}
```

- `Acquire::process()` → `Acquire::session()`。
- `Pool::new` 的闭包类型改成 `Future<Output = Option<Handle>>`；
  `acquire` 返回 `Result<Acquire, PoolError>`，空闲全坏/没有时
  `(self.new_session)().await.ok_or(PoolError::Create)?`。
- `acquire` 的探针调用改成 `(handle.liveness)().await`。
- `release_handle` 里 push 之前 `handle.idle_since = Instant::now();`。
- 新增：

  ```rust
  pub async fn idle_count(&self) -> usize { self.idle.lock().await.len() }

  /// 销毁空闲超过 `ttl` 的会话,但至少留 `POOL_MIN` 个。返回销毁数。
  pub async fn sweep_idle(&self, ttl: Duration) -> usize {
      let mut doomed = Vec::new();
      {
          let mut idle = self.idle.lock().await;
          let mut i = 0;
          while i < idle.len() {
              if idle.len() - doomed.len() <= POOL_MIN
                  || idle[i].idle_since.elapsed() < ttl {
                  i += 1;
              } else {
                  doomed.push(idle.remove(i).expect("i 在界内"));
              }
          }
      }
      let n = doomed.len();
      drop(doomed);          // 触发 destroy
      n
  }
  ```

  `POOL_MIN` 从写死改成 `Pool` 的字段 `min: usize`，`Pool::new(min, f)`；
  现有测试传 `1`。Task 7 的 `cfg.lsp_pool_min` 从这里接进来。

- `PoolError`：

  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum PoolError {
      #[error("起不了 RA 会话(镜像未就绪或 docker 不可用)")]
      Create,
  }
  ```

**3.3 新建 `playground/src/lsp/docker_exec.rs`：真实的 `RaCommand`。**

要点（每一条都对应一个已知坑）：

- `tokio::process::Command::new("docker").args(args)`，
  `.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())`，
  **`.kill_on_drop(true)`**（T3 的契约）。
- `spawn` 后立刻 `take()` 走 `stdin` / `stdout`；起一个任务把 stdout 读成 8 KiB 块
  塞进 `mpsc::UnboundedSender<Vec<u8>>`（`take_stdout` 返回它的接收端）。
  读端返回 `Ok(0)` 或 `Err` = 子进程结束 → drop sender（`RaProcess` 据此得到
  `ChildExited`）。
- `write`：`stdin.write_all(bytes)` + `flush`；stdin 已被取走或写失败 → `ExecError::Write`。
- `kill`：`child.start_kill()`，然后 `child.wait().await` 收尸（避免僵尸）。
- 三段共享状态用 `tokio::sync::Mutex`（`write` / `kill` 要 await）。

```rust
pub struct DockerExec {
    stdin: tokio::sync::Mutex<Option<tokio::process::ChildStdin>>,
    child: tokio::sync::Mutex<Option<tokio::process::Child>>,
    stdout_tx: std::sync::Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
}

impl DockerExec {
    pub fn new() -> Self { ... }
}
```

**3.4 `docker.rs` 加 `rm_args`：**

```rust
pub fn rm_args(name: &str) -> Vec<String> {
    vec!["rm".into(), "-f".into(), name.into()]
}
```

**3.5 新建 `playground/src/lsp/service.rs`。**

```rust
pub struct LspService {
    cfg: Arc<Config>,
    baker: Arc<Baker>,
    pool: Arc<Pool>,
    next: AtomicU64,
}
```

- `new(cfg, baker)`：`Pool::new(cfg.lsp_pool_min, move || { /* 见下 */ })`。
  闭包捕获 `Weak`/`Arc<LspService>` 会循环；改用一个小结构：
  `let maker = Arc::new(SessionMaker { cfg, baker, next });`，闭包只捕获 `maker`。
- `SessionMaker::make()`：
  1. `let tag = self.baker.status().serving_tag?;`
  2. `let name = format!("r2g-ra-{}", self.next.fetch_add(1, SeqCst));`
  3. `RunSpec { name, image: tag, src_dir: PathBuf::new(), out_dir: PathBuf::new(),
     workdir: RA_WORKDIR.into(), cpus: Some(2), memory: cfg.lsp_memory.clone(),
     pids_limit: 256, user: "1001:1001".into(), source_file: SOURCE_FILE.into() }`
     → `docker` + `run_detached_args(&spec)`，失败返回 `None`。
  4. `DockerExec::new()` → `RaProcess::start(exec, &name)`，失败 → `docker rm -f` 后 `None`。
  5. `Session::new` → `initialize()`，失败 → 清理后 `None`。
  6. `let mut ev = session.subscribe();` → `did_open(PLACEHOLDER)`，
     `PLACEHOLDER = "// playground: waiting for user source\n"`。
  7. 就绪判据：在 `cfg.lsp_boot_timeout` 内收到**第一条 `Event::Diagnostics` 或
     `Event::Tokens`** 即为就绪（谁先到都算）。收不到 → 清理后 `None`。
     > 计划原先只认 Diagnostics；占位内容零错误时 RA 未必发这条通知。Tokens 在
     > `did_open` 里被主动请求，是更可靠的兜底。若判断错了，代价是偶尔多等
     > 一个 boot timeout 再重建一次。
  8. `Some(Handle::new(proc, session, destroy).with_liveness({
         let s = Arc::clone(&session);
         move || { let s = Arc::clone(&s);
                   Box::pin(async move { s.ping(Duration::from_secs(2)).await.is_ok() }) }
     }))`
     `destroy` = `tokio::spawn` 跑 `docker rm -f <name>`（容器一删，exec 与 RA 一起没）。
- `acquire`：`self.pool.acquire().await`。
- `write_source(container, source)`：`docker exec -i <container> sh -c 'cat > /opt/ra/ws/demo/src/lib.rs'`，
  写 stdin 后等退出码；非 0 → `Err`。**必须在 `did_change` 之前 await 完成**。
- `spawn_background`：两个 `tokio::spawn`：
  - 补池：`loop { while pool.idle_count().await < cfg.lsp_pool_min { match pool.acquire().await { Ok(a) => a.release(true).await, Err(_) => sleep(5s).await } } sleep(500ms).await }`
    —— 走 `acquire` + `release` 就不需要给池加「直接塞一个进去」的口子。
  - 回收：`loop { sleep(30s); pool.sweep_idle(cfg.lsp_idle_ttl).await; }`（`ttl == 0` 时跳过）。

- [ ] **Step 4: 跑测试确认它们过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/lsp/ playground/src/docker.rs && git commit -m "feat(play): RA 服务运行时 —— 真实 exec、可复用会话、池的回收

三处集成缺口:
- RaCommand 之前只有测试里的假实现,补上真正起 docker exec 的 DockerExec。
- Session::events() 只能取一次,而池要跨连接复用;改成 broadcast::subscribe。
- 池的存活性检查直接 ping 裸管道,会和会话的 reader 抢同一根输出流;改成
  Handle::with_liveness 注入 Session::ping(走 pending 表)。

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 9b: 服务端接线

**Files:**
- Modify: `playground/src/server/http.rs`（`AppState` + 测试装配点）
- Modify: `playground/src/server/ws.rs`
- Modify: `playground/src/main.rs`
- Modify: `playground/src/lsp/pool.rs`（`container` 字段与访问器）
- Modify: `playground/src/lsp/service.rs`（建 Handle 时链上容器名）
- Modify: `playground/src/testing.rs`（需要时）

**Interfaces:**
- Consumes: Task 9 的 `LspService` / `Acquire` / `Session::subscribe`
- Produces: 端到端的 `LspOpen` → `LspReady` / `LspTokens` / `LspDiagnostic` / `LspCompletion`

- [ ] **Step 1: 写失败的测试**

**(a) 纯逻辑：LSP 关掉时 `LspOpen` 回错误、`hello` 报 `lsp_supported: false`。**

```rust
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
```

**(b) `hello.lsp_supported` 随 `AppState.lsp` 变化。** 用
`LspService::new(Arc::clone(&cfg), Baker::new(fake_builder, fake_gate))` 造一个
（它此刻不碰 docker），断言 `lsp_supported == true`。`Baker::new` 的签名是
`(Arc<dyn ImageBuilder>, Arc<dyn DiskGate>)`（`bake.rs:129`），测试里已有现成的假实现。

**(c) 集成（`#[ignore = "需要本机有 docker 与烘好的镜像"]`）**：真起一个会话，走
`LspOpen` → 收 `LspReady` → `LspChange` → 收 `LspDiagnostic`；断开后池里的会话
可被第二条连接复用。

- [ ] **Step 2: 跑测试确认它们失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground lsp_open_is_rejected
```

- [ ] **Step 3: 实现**

**3.1 `http.rs`：** `AppState` 加 `pub lsp: Option<Arc<crate::lsp::service::LspService>>`。
**所有** `AppState { .. }` 构造点补字段：`http.rs` 里 4 处（`state` / `any_origin_state` /
`webhook_state`）、`ws.rs` 里 2 处、`main.rs` 1 处。测试里默认 `lsp: None`。

**3.2 `ws.rs`：**

- `hello` 的 `lsp_supported: state.lsp.is_some()`（删掉那句 `// Task 9 接池后再按 AppState 判断`）。
- 连接内状态：

  ```rust
  struct LspConn {
      acq: crate::lsp::pool::Acquire,
      forwarding: JoinHandle<()>,
  }
  ```

- `LspOpen {}`：
  - `let Some(svc) = state.lsp.clone() else { send Error{Internal, "代码分析功能已关闭"} }`。
  - 已经有 `lsp_conn` → 忽略（幂等，不要重复取会话）。
  - `tokio::time::timeout(cfg.lsp_boot_timeout, svc.acquire())`；超时/失败 →
    `Error{Internal, "分析器起不来,稍后再试"}`。
  - `let mut ev = acq.session().subscribe();`，起转发任务：
    `loop { match ev.recv().await { Ok(Event::Tokens(d)) => tx.send(ServerMsg::LspTokens{data:d}),
       Ok(Event::Diagnostics(list)) => for d in list { tx.send(ServerMsg::LspDiagnostic{..}) },
       Ok(Event::Completed{request_id, items}) => tx.send(ServerMsg::LspCompletion{request_id, items}),
       Err(broadcast::error::RecvError::Lagged(_)) => continue, Err(Closed) => break } }`
  - **不在这里 didOpen**：待命会话在补池时已经 didOpen 过占位内容，同一个 URI
    再 didOpen 是 LSP 协议违规。用户的源码走 `LspChange` 的 `did_change`（全文替换）。
    > 设计稿的时序图写的是 LspOpen 时 didOpen(用户源码)；实现取「规范化」的
    > didChange。两者都达到「用用户源码替换文档」的效果，didChange 不违反协议。
  - 回 `ServerMsg::LspReady { file: SOURCE_FILE.to_string() }`。
- `LspChange { source }`：
  - 没有会话 → `Error{BadRequest, "先发 lsp_open"}`。
  - `svc.write_source(acq.container(), &source).await` —— 名字来自 `acq` 所在的容器。
    Task 9 的 `Handle` 没有存这个名字，**用一个不打扰现有测试的加法**：
    `Handle` 加 `container: String` 字段（默认空串）、
    `pub fn with_container(mut self, name: impl Into<String>) -> Self` 与
    `Acquire::container(&self) -> &str`；`SessionMaker::make` 里建 Handle 时链上
    `.with_container(name)`。不改 `Handle::new` 的签名，池的现有测试零改动。
  - 写盘成功后再 `acq.session().did_change(&source).await`；失败 → `Error{Internal}`。
- `LspCompletion { request_id, line, column }`：`session.request_completion(...)`，
  错误 → `Error{Internal}`。
- 循环末尾（与 job 的 abort 同一处）：`if let Some(c) = lsp_conn.take() {
      c.forwarding.abort();
      let alive = c.acq.session().is_alive();
      c.acq.release(alive).await;
  }`
  —— 会话死就销毁，活着就回池。

**3.3 `main.rs`：**

```rust
let lsp = if cfg.lsp_disabled {
    None
} else {
    let svc = crate::lsp::service::LspService::new(Arc::clone(&cfg), Arc::clone(&baker));
    Some(Arc::clone(&svc))
};
```
（`svc.spawn_background()` 在 `LspService::new` 里**不要**自动起 —— 测试要能造一个
不起后台任务的实例。在 `main.rs` 里显式调一次。）

- [ ] **Step 4: 跑测试确认它们过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/server/http.rs playground/src/server/ws.rs playground/src/main.rs playground/src/testing.rs playground/src/lsp/ && git commit -m "feat(play): LSP 接线 —— AppState、ws 分发、后台补池与回收

连接断开时必须释放池里的 RA 会话,否则 839 MB 挂在容器里不放。会话死
(alive=false)就销毁,活着回池。

DISABLED 时 AppState::lsp 为 None,ws 回 Error 让前端不挂 LSP 客户端。

Co-Authored-By: Claude Code <noreply@anthropic.com>"
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
[ ] `Hello.targets` 驱动的下拉框有两个选项:「超限模式」「经典模式」;
    默认超限,切换后点编译,产物确实按所选目标编译(日志里 --target 路径对得上)
[ ] 切到经典模式再切回来,两次都能编译成功(镜像里两个目标都烘过)
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

补全触发走 CodeMirror 的 `CompletionSource`：用户主动呼出补全（或输入触发字符）时，
前端自己生成一个 `request_id`（单调递增的 `u32`），发 `ClientMsg::LspCompletion
{ request_id, line, column }`，并在一个 `Map<request_id, resolve>` 里挂起这个 Promise；
收到 `ServerMsg::LspCompletion` 就按 `request_id` 取出并 resolve。请求发出后不要
无限等 —— 设一个 2s 的 `setTimeout` 兜底 resolve 空列表，否则 RA 卡住时补全弹窗会一直转圈。
一个请求在途时用户又敲了键：直接丢弃旧 Promise（resolve 空列表），不要排队。

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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
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

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 12: 超限 / 经典目标选择

**执行顺序**：Task 9 之后、Task 10 之前（Task 10 的前端选择器要用这里的 `targets` 列表）。

**Files:**
- Modify: `playground/src/docker.rs`（`RunSpec` 加 `target`，`container_command` 按它拼路径）
- Modify: `playground/src/runner.rs`（构造 `RunSpec` 时把 `job.target` 传进去）
- Modify: `playground/src/server/ws.rs`（`validate_submit` 放行 `classic-server`，`Hello.targets` 两条）
- Modify: `playground/Dockerfile`（烘焙时两个目标都编一遍）

**Interfaces:**
- Consumes: `JobSpec.target`（已有）
- Produces: `Hello.targets = ["beyond-server", "classic-server"]`

**背景**：协议里 `Submit { target: Option<String> }`、`JobSpec.target`、`Runner` 都已经有了，
只是 `container_command` 把 `beyond-server` 写死，`validate_submit` 也只放行它。

两个目标规格的差别只有 `pre-link-args`（`-gamemode beyond|classic`）和 `llvm-target`
（`genshin-unknown-{beyond,classic}-server`）。**不同的 target triple 意味着 `target/` 下是
两个独立目录**，依赖不会共享缓存 —— 所以镜像必须在烘焙时把两个都编一遍，否则第一次选经典
模式要冷编译整棵依赖树。

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn container_command_uses_the_selected_target() {
    let mut s = spec();
    s.target = "classic-server".into();
    let cmd = container_command(&s);
    assert!(cmd.contains("--target ./genshin-unknown-classic-server.json"), "{cmd}");
    assert!(cmd.contains("cp target/genshin-unknown-classic-server/release/rust2genshin_demo.gia /out/"), "{cmd}");
    assert!(!cmd.contains("beyond-server"), "{cmd}");
}

#[test]
fn validate_submit_accepts_both_targets() {
    assert!(validate_submit("fn main() {}", Some("beyond-server")).is_ok());
    assert!(validate_submit("fn main() {}", Some("classic-server")).is_ok());
    assert!(validate_submit("fn main() {}", Some("nope")).is_err());
}

#[test]
fn hello_lists_both_targets() {
    // 断言 WS 欢迎消息里的 targets 是 ["beyond-server", "classic-server"]
}
```

- [ ] **Step 2: 跑测试确认它失败**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground
```

- [ ] **Step 3: 实现**

- `RunSpec` 加 `pub target: String`；**四处**构造都补上：`docker.rs` 的测试 `spec()` /
  `ra_spec()`、`runner.rs:191`、`lsp/service.rs:165`（Task 9 新增的 RA 容器）。后两处虽然不走
  `container_command`，也给 `"beyond-server"` 并在注释里说明它用不到。
- `container_command` 用 `spec.target` 同时替换 `--target ./genshin-unknown-{t}.json` 与
  `cp target/genshin-unknown-{t}/release/rust2genshin_demo.gia /out/` 两处。**两处必须同一个
  变量**，写死其中一处就是产物路径对不上。
- `validate_submit` 的白名单加 `classic-server`，错误文案改成「目前只有 beyond-server / classic-server」。
- `Hello.targets` 改成两条。
- `Dockerfile` 烘焙段加一行 `cargo run -p build-demo --bin classic-server`（在 beyond 那行之后）。

- [ ] **Step 4: 跑测试确认它过**

```
cd C:/projects/rust2genshin && cargo test -p rust2genshin-playground
```

- [ ] **Step 5: 提交**

```bash
cd C:/projects/rust2genshin && git add playground/src/docker.rs playground/src/runner.rs playground/src/server/ws.rs playground/Dockerfile && git commit -m "feat(play): 目标可选超限 / 经典,两个都在镜像里烘好

两个 target spec 的 triple 不同,cargo 在 target/ 下按 triple 分目录 ——
经典模式无法复用超限模式的依赖缓存。所以镜像烘焙时两个都编一遍,
否则第一次选经典要冷编译整棵依赖树。

container_command 里 --target 和 cp 的路径必须同一个变量,写死一处就是
产物路径对不上。

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

**注意**：前端的选择器在 Task 10 做（`Hello.targets` 驱动下拉框，`Submit` 带上选中的值）；
LSP 链路**不感知目标** —— 分析用的 workspace 与 gamemode 无关。

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
