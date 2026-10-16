//! 跑一个任务容器。
//!
//! 退出码非 0 → 编译失败;**退出码 0 但没有 `/out/*.gia` 也判失败** ——
//! 那会给出一个指向不存在文件的下载链接,或者更糟:把上一次的产物当成
//! 这一次的结果。超时是唯一需要服务自己解释的失败(要带上已用时),其余
//! 一律把 cargo 的原文推给前端,服务不解析。

use std::{sync::Arc, time::Duration};

use thiserror::Error;

use crate::{
    docker::{FrameError, Stream},
    job::JobPaths,
};

#[derive(Debug, Clone)]
pub struct JobSpec {
    pub task: String,
    pub source: String,
    pub target: String,
    pub image_tag: String,
    pub image_commit: String,
    pub paths: JobPaths,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
    pub artifact_bytes: u64,
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("起容器失败:{0}")]
    Spawn(#[from] std::io::Error),
    #[error("容器退出码 {code:?}，但没有可用的产物")]
    NoArtifact { code: Option<i32> },
    #[error("超过 {0:?} 还没跑完,已经强制结束")]
    Timeout(Duration),
    #[error("解析容器输出失败:{0}")]
    Decode(#[from] FrameError),
}

pub type LogSink = Arc<dyn Fn(Stream, &str) + Send + Sync>;

/// 只有"退出码 0 且产物在"才算成功。产物大小由调用方给出(`None` = 文件不在)。
pub fn verdict(code: Option<i32>, artifact_bytes: Option<u64>) -> Result<u64, RunnerError> {
    match (code, artifact_bytes) {
        (Some(0), Some(n)) => Ok(n),
        (code, _) => Err(RunnerError::NoArtifact { code }),
    }
}

#[async_trait::async_trait]
pub trait JobRunner: Send + Sync {
    /// 跑完返回结果。`log` 会在每一行输出到达时被调用 —— 实现里要把
    /// stdout / stderr 分开(靠 8 字节帧),前端就能把它们染成不同颜色。
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError>;
}

/// 任务容器里**固定**的那几个参数。做成一个结构体传进去,免得
/// "哪个任务偷偷少挂了一个 `--network none`"这种事发生。
#[derive(Debug, Clone)]
pub struct DockerOptions {
    pub docker: String,
    pub workdir: String,
    pub cpus: u32,
    pub memory: String,
    pub pids_limit: u32,
    pub user: String,
    pub timeout: Duration,
    pub container_prefix: String,
}

impl Default for DockerOptions {
    fn default() -> Self {
        Self {
            docker: "docker".into(),
            workdir: "/opt/rust2genshin".into(),
            cpus: 2,
            memory: "1536m".into(), // 写 1536m 不写 1.5g:docker 对小数容量不友好
            pids_limit: 256,
            user: "1001".into(),
            timeout: Duration::from_secs(15),
            container_prefix: "r2g".into(),
        }
    }
}

/// 强制停止路径本身的封顶。
///
/// "给卡住封顶"的动作自己不能没有顶:daemon 无响应时 `docker kill` 会一直等,
/// 于是 `run` 永远不返回、永远占着队列额度。
const FORCED_STOP_TIMEOUT: Duration = Duration::from_secs(5);

pub struct DockerRunner {
    opts: DockerOptions,
}

impl DockerRunner {
    pub fn new(opts: DockerOptions) -> Self {
        Self { opts }
    }

    async fn docker(&self, args: &[String]) -> std::io::Result<std::process::Output> {
        tokio::process::Command::new(&self.opts.docker)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .await
    }
}

/// 容器的兜底守卫:**只要容器没被证实退出,`Drop` 就补一发 `docker kill`**。
///
/// 为什么必须有它:任务 future 会被上层 `JoinHandle::abort()` 取消(用户点
/// cancel / 关标签页)。abort 是**直接把 future 丢掉** —— `run` 里那些
/// `docker kill` 一行都不会跑,而 `--rm` 只在容器**退出时**清理。一个
/// `loop {}` 的容器被丢下就会一直跑,一直占着 2 个 CPU + 1.5GB。
///
/// 用 `std::process::Command` 而不是 tokio:`Drop` 里没法 await,而这一发
/// 只要求"发出去、有人收尸",不要求看着它结束。
struct ContainerGuard {
    docker: String,
    name: String,
    /// 已经确认容器退出了(正常 `wait()` 返回)。
    exited: std::sync::atomic::AtomicBool,
}

impl ContainerGuard {
    fn new(docker: &str, name: &str) -> Self {
        Self {
            docker: docker.to_string(),
            name: name.to_string(),
            exited: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// 容器确认退出 —— 之后不再补刀。
    fn mark_exited(&self) {
        self.exited
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for ContainerGuard {
    fn drop(&mut self) {
        use std::process::{Command, Stdio};
        if self.exited.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        // best-effort:这一步没有第二条路可走,失败只能吭声。
        match Command::new(&self.docker)
            .args(crate::docker::kill_args(&self.name))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut c) => {
                tracing::warn!(
                    target: "runner",
                    "容器 {} 未确认退出,已补发 docker kill",
                    self.name
                );
                // 收尸:不 wait 会留僵尸。短命线程,不值得上线程池。
                std::thread::spawn(move || {
                    let _ = c.wait();
                });
            }
            Err(e) => tracing::warn!(
                target: "runner",
                "容器 {} 未确认退出,补发 docker kill 失败:{e};容器可能残留",
                self.name
            ),
        }
    }
}

#[async_trait::async_trait]
impl JobRunner for DockerRunner {
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError> {
        let name = format!("{}-{}", self.opts.container_prefix, job.task);
        let spec = crate::docker::RunSpec {
            name: name.clone(),
            image: job.image_tag.clone(),
            src_dir: job.paths.src_dir.clone(),
            out_dir: job.paths.out_dir.clone(),
            workdir: self.opts.workdir.clone(),
            cpus: self.opts.cpus,
            memory: self.opts.memory.clone(),
            pids_limit: self.opts.pids_limit,
            user: self.opts.user.clone(),
            source_file: crate::protocol::SOURCE_FILE.to_string(),
        };
        let mut child = tokio::process::Command::new(&self.opts.docker)
            .args(crate::docker::run_args(&spec))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // 从这一刻起容器可能已经活着了,兜底守卫开始值班。
        let guard = ContainerGuard::new(&self.opts.docker, &name);

        // 容器自己的 stdout 是分帧的(8 字节头 + 载荷),docker CLI 自己的
        // stderr 是裸文本 —— 两条分开读。
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let loud = Arc::clone(&log);
        let stdout_task = tokio::spawn(async move {
            read_frames(stdout, loud).await
        });
        let lerr = Arc::clone(&log);
        let stderr_task = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                lerr(Stream::Stderr, &format!("{l}\n"));
            }
        });

        // 计时覆盖 `docker run` 全程:从 `spawn()` 之后起算,到 `wait()` 返回为止,
        // 中间的容器创建 / 启动(约 1 秒)也算在用户的预算里。`Command::spawn`
        // 本身不受超时保护 —— 极罕见地卡在 fork/exec 上不会被这里掐掉。
        let started = std::time::Instant::now();
        let mut timed_out = false;
        let status = match tokio::time::timeout(self.opts.timeout, child.wait()).await {
            Ok(Ok(s)) => {
                // 只有拿到退出码才算"容器确认退出"。`child.wait()` 成功但
                // `code()` 为 `None` 时,docker CLI 是被信号杀死的 —— daemon
                // 侧的容器可能还活着。这时不卸甲,交给守卫的 Drop 去补刀。
                if s.code().is_some() {
                    guard.mark_exited();
                }
                Some(s)
            }
            // `wait()` 自己报错:读不到容器状态,不卸甲,让守卫兜底。
            Ok(Err(e)) => return Err(RunnerError::Spawn(e)),
            Err(_) => {
                timed_out = true;
                // 光 kill CLI 不够:容器由 daemon 持着,CLI 只是个 attached
                // client。必须先 `docker kill`,再收 CLI。
                //
                // 两层封顶:这一发自己不许无限等;清不掉就**不卸甲**,
                // 交给守卫的 Drop 再补一发。
                match tokio::time::timeout(
                    FORCED_STOP_TIMEOUT,
                    self.docker(&crate::docker::kill_args(&name)),
                )
                .await
                {
                    Ok(Ok(out)) if out.status.success() => guard.mark_exited(),
                    Ok(Ok(out)) => tracing::warn!(
                        target: "runner",
                        "docker kill {name} 退出码 {:?};交给兜底守卫重试",
                        out.status.code()
                    ),
                    Ok(Err(e)) => tracing::warn!(
                        target: "runner",
                        "docker kill {name} 起不来:{e};交给兜底守卫重试"
                    ),
                    Err(_) => tracing::warn!(
                        target: "runner",
                        "docker kill {name} 超过 {FORCED_STOP_TIMEOUT:?};交给兜底守卫重试"
                    ),
                }
                let _ = child.kill().await;
                let _ = tokio::time::timeout(FORCED_STOP_TIMEOUT, child.wait()).await;
                None
            }
        };
        // `elapsed` 在这里取:报的是"容器跑了多久",**不含**后面两个读任务
        // 收尾(drain 缓冲)的时间 —— 刻意如此。
        let elapsed = started.elapsed();
        // 取回 stdout 帧读取任务的结果 —— 解码失败要上报,不能像 stderr 那样丢掉。
        let (frames_res, _) = tokio::join!(stdout_task, stderr_task);

        // 判定顺序是**超时 → 解码 → verdict**,两个理由:
        // 1. 超时先判:被 kill 的容器会在帧中间截断,若先判解码,一次正常的
        //    超时会被误报成"读容器输出失败"。
        // 2. 解码排在 verdict 前:帧坏意味着**我们与 docker 之间的输出流已经
        //    失步**(分帧是 docker CLI/daemon 做的,不是容器自己写的),此后
        //    到达的内容都不该信。产物大小另有校验、失败是 fail-closed,所以
        //    宁可显式报一次解码错,也不拿"大概对得上"的产物继续。
        if timed_out {
            return Err(RunnerError::Timeout(elapsed));
        }
        match frames_res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(RunnerError::Decode(e)),
            // 读取任务自身 panic / abort:只为丢一条日志流就废掉一整次编译
            // 不值得,忽略它、继续走 verdict。
            Err(_join) => {}
        }

        // 产物已经落在宿主挂进来的 out/ 上;容器由上面的守卫保证会被收掉
        // (`--rm` 只在容器**退出**时清理,杀不掉的容器它管不了)。
        let bytes = std::fs::metadata(&job.paths.artifact).ok().map(|m| m.len());
        let code = status.as_ref().and_then(|s| s.code());
        let n = verdict(code, bytes)?;
        Ok(RunOutcome {
            exit_code: code,
            elapsed,
            artifact_bytes: n,
        })
    }
}

async fn read_frames<R>(r: R, log: LogSink) -> Result<(), FrameError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut r = r;
    let mut dec = crate::docker::FrameDecoder::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let n = r.read(&mut buf).await.map_err(|e| FrameError::Io(e.to_string()))?;
        if n == 0 {
            return Ok(());
        }
        // `push` 出错就意味着 decoder 作废(粘性错误,见 `docker.rs`),
        // `?` 让这里立刻停止喂数据、把这条流按终止处理。
        for f in dec.push(&buf[..n])? {
            log(f.stream, &String::from_utf8_lossy(&f.data));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_exit_with_artifact_is_ok() {
        // `RunnerError` 里有 `std::io::Error`(非 `PartialEq`),`Result` 整体比不了,
        // 所以取 `Ok` 的那一侧比较 —— 断言的意思不变。
        assert_eq!(verdict(Some(0), Some(15860)).unwrap(), 15860);
    }

    #[test]
    fn nonzero_exit_is_failure_even_with_artifact() {
        // cargo 有时会先写产物再报错(后端自己 warn/err 也算非 0)。
        match verdict(Some(101), Some(100)) {
            Err(RunnerError::NoArtifact { code }) => assert_eq!(code, Some(101)),
            other => panic!("期望 NoArtifact,得到 {other:?}"),
        }
    }

    #[test]
    fn zero_exit_without_artifact_is_failure() {
        // 这条抓的就是"cargo 判 fresh、跳过编译"那种静默错答案。
        match verdict(Some(0), None) {
            Err(RunnerError::NoArtifact { code }) => assert_eq!(code, Some(0)),
            other => panic!("期望 NoArtifact,得到 {other:?}"),
        }
    }

    #[test]
    fn killed_container_has_no_exit_code() {
        assert!(matches!(
            verdict(None, None),
            Err(RunnerError::NoArtifact { code: None })
        ));
    }

    #[test]
    fn timeout_says_so_instead_of_blaming_missing_output() {
        // 超时会走到 verdict 之外的那条分支(见 DockerRunner)。这条单测钉的是
        // 变体自己的契约:报的是"超时了多少",不是"没有产物"。
        let e = RunnerError::Timeout(Duration::from_secs(15));
        assert!(e.to_string().contains("15"), "{e}");
    }
}
