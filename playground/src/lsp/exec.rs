//! RA 会话的 exec 抽象 —— 把「起 docker exec 子进程」和「和 RA 说话」拆开,
//! 真实实现用 `tokio::process::Command` 起子进程,假实现只记录调用,测试
//! 不碰真 docker。
//!
//! 见 `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md`。

use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;

/// exec 子进程坏在哪里,放在一个枚举里,而不是塞一堆 `Box<dyn Error>`。
///
/// 与 `frame::FrameError` 是两回事:那个是 LSP 帧的字节错,这个是**和子进程
/// 沟通**层的错(起不来 / 写不进 / 拼不上)。拆开是因为修复路径不一样:
/// 帧坏 = 重连会话,子进程坏 = 起新实例。
#[derive(Debug, Error)]
pub enum ExecError {
    #[error("起 RA 子进程失败:{0}")]
    Spawn(String),
    // 真实实现在写 stdin 时如果对端已关(RA 已死),会构造 `Write`。
    // 假实现 / 本测试用不到 —— 留着是因为返回类型要覆盖这条路径,
    // 现在没用上不等于将来没用。
    #[allow(dead_code)]
    #[error("写 RA 的 stdin 失败:{0}")]
    Write(String),
    /// 真 RA 应当回应一条 ping,沉默就算失败 —— 表示容器里 RA 没起来或挂了。
    #[error("RA ping 超时({0:?} 内没有任何回应)")]
    PingTimeout(Duration),
}

/// 「和 RA 子进程握手」这件事的抽象。
///
/// 真实实现起 `tokio::process::Command`(`stdin/stdout` 都 `Stdio::piped()`),
/// 再 `tokio::spawn` 一个任务把 `child.stdout` 读成块塞进 channel;
/// 假实现只把调用记下来,测试里用。
///
/// `take_stdout` 默认返回 `None`(就是「没有真输出」):假实现有这个默认就够,
/// 真实实现 override 它给出真 channel。`RaProcess::start` 在 `spawn` 之后
/// 立刻取走,所以「只能取一次」不是问题。
#[async_trait]
pub trait RaCommand: Send + Sync + 'static {
    async fn spawn(&self, args: &[String]) -> Result<(), ExecError>;
    async fn write(&self, bytes: &[u8]) -> Result<(), ExecError>;
    async fn kill(&self);

    /// 拿出 stdout 读端。**默认 None**(假实现 / 没有真输出)。
    /// 真实实现在 `spawn` 之后能让这个方法返回真 channel,这里取走后
    /// `recv` 才有数据可读。
    fn take_stdout(&self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
        None
    }
}

/// 一个连上 RA 的会话。**`Recv` 必须 `&self`** —— 池里会并发调它
/// (请求分发到不同会话),`&mut self` 没法让 `Arc<RaProcess>` 共享。
pub struct RaProcess {
    cmd: std::sync::Arc<dyn RaCommand>,
    /// `spawn` 之后从 `cmd.take_stdout()` 拿到。可能 `None`(假实现)。
    /// `tokio::sync::Mutex`,不是 `std::sync::Mutex`:`recv` 要 `await` 锁。
    stdout: tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>,
    /// 拆帧器,在 `recv` 里持续喂读到的字节。
    decoder: std::sync::Mutex<super::frame::FrameDecoder>,
}

impl RaProcess {
    /// 起一个 RA 会话。具体起法交给 `cmd.spawn`,这里只组参数。
    pub async fn start(
        cmd: std::sync::Arc<dyn RaCommand>,
        container_name: &str,
    ) -> Result<Self, ExecError> {
        // 容器名是「执行环境」的一部分,不是 RA 自己的参数 —— RA 看到的就
        // 是「我现在在某个容器里、给我 LSP 连接」,容器是 exec_args 里给的。
        let args = crate::docker::exec_args(container_name, "rust-analyzer");
        cmd.spawn(&args).await?;
        let stdout = cmd.take_stdout();
        Ok(Self {
            cmd,
            stdout: tokio::sync::Mutex::new(stdout),
            decoder: std::sync::Mutex::new(super::frame::FrameDecoder::new()),
        })
    }

    /// 发一条 LSP 消息:**长度按字节算**(`frame::encode` 内部就这语义),
    /// UTF-8 下中文一字 3 字节 —— 那是 `frame::encode` 的事,这里只喂字符串。
    pub async fn send(&self, json: &str) -> Result<(), ExecError> {
        let bytes = super::frame::encode(json);
        self.cmd.write(&bytes).await
    }

    /// 从 stdout 拿**第一条完整**消息。超时返回 `Ok(None)` —— 调用方看
    /// `None` 就当「RA 没回」(具体是不是 ping 失败,由 `ping` 自己定)。
    ///
    /// 拿不到 stdout channel(假实现)直接等到超时、返回 `Ok(None)`。
    pub async fn recv(&self, timeout: Duration) -> Result<Option<String>, ExecError> {
        let chunk = {
            let mut guard = self.stdout.lock().await;
            let Some(rx) = guard.as_mut() else {
                tokio::time::sleep(timeout).await;
                return Ok(None);
            };
            // 在 channel 读和定时器之间选,不会因为 rx 没数据就吃掉一次 timeout。
            match tokio::time::timeout(timeout, rx.recv()).await {
                Ok(Some(c)) => c,
                Ok(None) => return Ok(None),
                Err(_) => return Ok(None),
            }
        };
        // 锁先放,decoder 是 std Mutex(短临界区),普通 lock 就行。
        let mut dec = self.decoder.lock().unwrap();
        let frames = dec
            .push(&chunk)
            .map_err(|e| ExecError::Spawn(format!("LSP 帧解码失败:{e:?}")))?;
        Ok(frames
            .into_iter()
            .next()
            .map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    /// ping:发一条请求然后等**任意**一条响应。`Ok(None)`(沉默)算失败。
    ///
    /// 用 `initialize` 而不是 `shutdown`:RA 一上来就在等 `initialize`,
    /// 发别的请求会排队到 initialize 完成之后才被处理,ping 反而更慢。
    pub async fn ping(&self, timeout: Duration) -> Result<(), ExecError> {
        // 文案写死,内容不重要 —— RA 不验它的合法性,只要是一帧就能路由到。
        self.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .await?;
        match self.recv(timeout).await? {
            Some(_) => Ok(()),
            None => Err(ExecError::PingTimeout(timeout)),
        }
    }

    /// 主动关。`Drop` 也会关,这是给优雅路径用的 —— 留接口是因为将
    /// 来可能要在 abort 之前发 `shutdown` 请求。
    #[allow(dead_code)]
    pub async fn shutdown(&self) {
        self.cmd.kill().await;
    }
}

/// 会话被 abort(`tokio::test` 跑完 / 任务被取消)时,进程没人显式关。
/// 残留的 RA 在容器里占 839 MB 不放 —— 显式起一个 tokio task 至少给个
/// `kill` 的机会。
impl Drop for RaProcess {
    fn drop(&mut self) {
        let cmd = std::sync::Arc::clone(&self.cmd);
        tokio::spawn(async move {
            cmd.kill().await;
        });
    }
}

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

    // 简报原文是 `impl RaCommand for Arc<FakeDocker>`,但 `Arc<FakeDocker>`
    // 不能 coerce 进 `Arc<dyn RaCommand>`(后者要求 `FakeDocker: RaCommand`),
    // 所以实现改在 `FakeDocker` 本身。语义没变:方法体只用 `self` 的字段。
    #[async_trait::async_trait]
    impl RaCommand for FakeDocker {
        async fn spawn(&self, args: &[String]) -> Result<(), super::ExecError> {
            self.spawned.lock().unwrap().push(args.to_vec());
            Ok(())
        }
        async fn write(&self, bytes: &[u8]) -> Result<(), super::ExecError> {
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
        let p = RaProcess::start(Arc::clone(&fake) as Arc<dyn RaCommand>, "r2g-ra-1")
            .await
            .unwrap();
        let spawned = fake.spawned.lock().unwrap();
        assert_eq!(spawned[0], vec!["exec", "-i", "r2g-ra-1", "sh", "-c", "rust-analyzer"]);
        drop(p);
    }

    #[tokio::test]
    async fn send_frames_the_message() {
        let fake = Arc::new(FakeDocker::default());
        let p = RaProcess::start(Arc::clone(&fake) as Arc<dyn RaCommand>, "r2g-ra-1")
            .await
            .unwrap();
        p.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .await
            .unwrap();
        let w = fake.written.lock().unwrap();
        // 那条 JSON 是 58 字节 —— 手数容易错,别写死一个猜的数,
        // 否则失败会被误读成「拆帧算错了字节」。
        assert!(w[0].starts_with("Content-Length: 58\r\n\r\n"), "实际:{}", &w[0][..40]);
    }

    #[tokio::test]
    async fn ping_times_out_on_silence() {
        let fake = Arc::new(FakeDocker::default());
        let p = RaProcess::start(fake as Arc<dyn RaCommand>, "r2g-ra-1")
            .await
            .unwrap();
        assert!(p.ping(std::time::Duration::from_millis(50)).await.is_err());
    }
}
