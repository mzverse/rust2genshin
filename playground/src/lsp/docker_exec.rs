//! [`RaCommand`] 的真实实现:每条 RA 会话是一个 `docker exec -i` 子进程。
//!
//! 为什么是 exec 而不是 attach:exec 的 stdin/stdout 是**裸字节管道**,
//! LSP 的 `Content-Length` 帧原样穿过;`docker run` 的 attach 协议会套
//! 8 字节复用头(`docker::FrameDecoder` 解的那种),RA 的帧就废了。
//!
//! 三个坑,各自对应一段代码:
//!
//! 1. **`kill_on_drop(true)`**:`RaProcess::Drop` 只是 `tokio::spawn` 一个
//!    kill,运行时正在关闭时那个任务不会被调度;子进程会留在容器里吃掉
//!    839 MB。`kill_on_drop` 是最后的兜底。
//! 2. **stdout 读完/读错时 drop sender**:`RaProcess` 靠 channel 关闭得到
//!    `ExecError::ChildExited`;sender 多活一份,它就永远等不到「RA 死了」。
//! 3. **stdin 写失败 = 对端已关**:转成 `ExecError::Write`,别把 IO 错误
//!    当成功咽下去。

use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use crate::lsp::exec::{ExecError, RaCommand};

/// 一次 `docker exec -i ... rust-analyzer` 子进程。
pub struct DockerExec {
    /// `spawn` 后从 `child.stdin` 取走。`None` = 还没起 / 已被取走。
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    /// 留着做 `kill` + `wait()` 收尸;`kill_on_drop` 兜底不走这里。
    child: tokio::sync::Mutex<Option<Child>>,
    /// stdout 的读端,`take_stdout` 取走一次。
    stdout: std::sync::Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>,
}

impl DockerExec {
    pub fn new() -> Self {
        Self {
            stdin: tokio::sync::Mutex::new(None),
            child: tokio::sync::Mutex::new(None),
            stdout: std::sync::Mutex::new(None),
        }
    }
}

impl Default for DockerExec {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RaCommand for DockerExec {
    async fn spawn(&self, args: &[String]) -> Result<(), ExecError> {
        let mut child = Command::new("docker")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // T3 的契约:运行时关闭 / 任务被 abort 时,只有这条兜底能
            // 保证 RA 不留在容器里。
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ExecError::Spawn(format!("docker 起不来:{e}")))?;

        let stdin = match child.stdin.take() {
            Some(s) => s,
            None => {
                let _ = child.start_kill();
                return Err(ExecError::Spawn("docker 子进程没有 stdin 管道".into()));
            }
        };
        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.start_kill();
                return Err(ExecError::Spawn("docker 子进程没有 stdout 管道".into()));
            }
        };

        // 读端在独立任务里把裸字节按 8 KiB 块推进 channel。sender 只在这里
        // 存一份 —— 读端结束(`Ok(0)` / `Err`)时它随任务 drop,channel 关闭,
        // `RaProcess::recv` 据此返回 `ChildExited`。
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            // RaProcess 已经不要了 —— 收掉读端,让 docker 的
                            // 管道尽早关闭。
                            break;
                        }
                    }
                }
            }
        });
        *self
            .stdout
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic") = Some(rx);

        *self.stdin.lock().await = Some(stdin);
        *self.child.lock().await = Some(child);
        Ok(())
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), ExecError> {
        let mut guard = self.stdin.lock().await;
        let Some(stdin) = guard.as_mut() else {
            return Err(ExecError::Write("stdin 已被取走或子进程未启动".into()));
        };
        stdin
            .write_all(bytes)
            .await
            .map_err(|e| ExecError::Write(e.to_string()))?;
        stdin
            .flush()
            .await
            .map_err(|e| ExecError::Write(e.to_string()))
    }

    async fn kill(&self) {
        let mut guard = self.child.lock().await;
        if let Some(child) = guard.as_mut() {
            // 已经退出时 `start_kill` 返回 Err —— 无所谓,`wait` 仍然收尸。
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }

    fn take_stdout(&self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
        self.stdout
            .lock()
            .expect("Mutex 不该中毒:持锁段不 panic")
            .take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `take_stdout` 只能取一次,且取走的是 `spawn` 创建的那个 channel。
    /// 不碰真 docker:直接构造并手动塞一个 rx,模拟 spawn 之后的形状。
    #[tokio::test]
    async fn take_stdout_is_one_shot() {
        let exec = DockerExec::new();
        {
            let (tx, rx) = mpsc::unbounded_channel();
            *exec
                .stdout
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic") = Some(rx);
            tx.send(b"hello".to_vec()).unwrap();
        }
        let mut rx = exec.take_stdout().expect("第一次该拿到读端");
        assert_eq!(rx.recv().await, Some(b"hello".to_vec()));
        assert!(exec.take_stdout().is_none(), "第二次必须是 None");
    }

    #[tokio::test]
    async fn write_before_spawn_is_a_write_error() {
        let exec = DockerExec::new();
        match exec.write(b"x").await {
            Err(ExecError::Write(_)) => {}
            other => panic!("期望 ExecError::Write,实际:{other:?}"),
        }
    }
}
