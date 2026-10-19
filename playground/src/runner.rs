//! 跑一个任务容器。
//!
//! 退出码非 0 → 编译失败;**退出码 0 但取不到产物也判失败** ——
//! 那会把一次「其实没编出东西」的编译报成成功。超时是唯一需要服务自己
//! 解释的失败(要带上已用时),其余一律把 cargo 的原文推给前端,服务不解析。
//!
//! 产物**不落宿主盘**:容器跑完(且**没有** `--rm`)之后用
//! `docker cp <容器>:<路径> -` 把 tar 流从 stdout 读回来,解出 `.gia`
//! 的字节直接交给上层推给前端。这样服务不需要 `out/` 目录、不留副本,
//! 也没有一条能被猜 id 读走的 HTTP 路由。

use std::{io::Read, sync::Arc, time::Duration};

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
    /// `.gia` 的字节。**已经在内存里** —— 上层直接推给前端,不再落盘。
    pub artifact: Vec<u8>,
}

impl RunOutcome {
    pub fn artifact_bytes(&self) -> u64 {
        self.artifact.len() as u64
    }
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
    #[error("从容器里取产物失败:{0}")]
    Fetch(String),
}

pub type LogSink = Arc<dyn Fn(Stream, &str) + Send + Sync>;

/// 只有"退出码 0 且产物取到了"才算成功。
pub fn verdict(code: Option<i32>, artifact: Option<Vec<u8>>) -> Result<Vec<u8>, RunnerError> {
    match (code, artifact) {
        (Some(0), Some(bytes)) if !bytes.is_empty() => Ok(bytes),
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
    /// `None` = 不给任务容器加 CPU 限额。见 `docker::RunSpec::cpus` 的
    /// 注释:宿主的 cgroup 不支持 cpu 配额时,`--cpus` 会让 docker 拒掉整个 `run`。
    pub cpus: Option<u32>,
    /// `None` = 不给任务容器加内存限额。见 `docker::RunSpec::memory` 的注释:
    /// cgroup v1 不支持 kmem 时,`--memory` 会让 docker 整个 run 失败。
    pub memory: Option<String>,
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
            cpus: Some(2),
            // 写 1536m 不写 1.5g:docker 对小数容量不友好
            memory: Some("1536m".into()),
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

/// 容器的兜底守卫:**只要容器没被证实收干净,`Drop` 就补一发 `docker rm -f`**。
///
/// 为什么必须有它:任务 future 会被上层 `JoinHandle::abort()` 取消(用户点
/// cancel / 关标签页)。abort 是**直接把 future 丢掉** —— `run` 里那条
/// `docker rm` 一行都不会跑,而任务容器**故意不带 `--rm`**(产物要等
/// `docker cp` 之后才取)。一个 `loop {}` 的容器被丢下就会一直跑,一直占着
/// 2 个 CPU + 1.5GB,还在镜像里留一个容器条目。
///
/// 用 `std::process::Command` 而不是 tokio:`Drop` 里没法 await,而这一发
/// 只要求"发出去、有人收尸",不要求看着它结束。
struct ContainerGuard {
    docker: String,
    name: String,
    /// 容器已经被证实收掉(正常 `wait()` 后我们主动 `docker rm -f` 过)。
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

    /// 容器确认收干净 —— 之后不再补刀。
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
            .args(crate::docker::rm_args(&self.name))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut c) => {
                tracing::warn!(
                    target: "runner",
                    "容器 {} 未确认收掉,已补发 docker rm -f",
                    self.name
                );
                // 收尸:不 wait 会留僵尸。短命线程,不值得上线程池。
                std::thread::spawn(move || {
                    let _ = c.wait();
                });
            }
            Err(e) => tracing::warn!(
                target: "runner",
                "容器 {} 未确认收掉,补发 docker rm -f 失败:{e};容器可能残留",
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
            workdir: self.opts.workdir.clone(),
            target: job.target.clone(),
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
                // 这里**不卸甲**:守卫现在管的是「容器有没有被收掉」,而
                // 容器不带 `--rm`,`docker run` 正常退出后条目还在,得由
                // 后面那次显式 `docker rm -f` 才算收干净。`code()` 为
                // `None` 更是明显没走正常路径。
                Some(s)
            }
            // `wait()` 自己报错:读不到容器状态,不卸甲,让守卫兜底。
            Ok(Err(e)) => return Err(RunnerError::Spawn(e)),
            Err(_) => {
                timed_out = true;
                // 光 kill CLI 不够:容器由 daemon 持着,CLI 只是个 attached
                // client。必须先让 daemon 把容器收掉,再收 CLI。
                //
                // 两层封顶:这一发自己不许无限等;清不掉就**不卸甲**,
                // 交给守卫的 Drop 再补一发。
                match tokio::time::timeout(
                    FORCED_STOP_TIMEOUT,
                    self.docker(&crate::docker::rm_args(&name)),
                )
                .await
                {
                    Ok(Ok(out)) if out.status.success() => guard.mark_exited(),
                    Ok(Ok(out)) => tracing::warn!(
                        target: "runner",
                        "docker rm -f {name} 退出码 {:?};交给兜底守卫重试",
                        out.status.code()
                    ),
                    Ok(Err(e)) => tracing::warn!(
                        target: "runner",
                        "docker rm -f {name} 起不来:{e};交给兜底守卫重试"
                    ),
                    Err(_) => tracing::warn!(
                        target: "runner",
                        "docker rm -f {name} 超过 {FORCED_STOP_TIMEOUT:?};交给兜底守卫重试"
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

        // 产物在容器自己的文件系统里 —— 现在去取。**取完才算成功**,
        // 顺序不能反:先 rm 容器,产物就跟着没了。
        //
        // 只在容器确实停了之后取:cp 报"容器不在运行"比报"文件不存在"更难懂,
        // 而 verdict 已经会把退出码非 0 的那次判成失败。
        let code = status.as_ref().and_then(|s| s.code());
        let artifact = match self.fetch_artifact(&name, &spec).await {
            Ok(bytes) => Some(bytes),
            Err(e) => {
                // 取不出来本身不是最终结论:退出码非 0 时真正的错因是编译失败,
                // 把 verdict 的 `NoArtifact { code }` 留给上层,别抢它的台词。
                tracing::warn!(target: "runner", "取产物失败:{e}");
                None
            }
        };

        // 收容器。守卫的 Drop 是兜底,这里是正常路径 —— 成功了就卸甲,
        // 不让每次编译都在日志里留一条「已补发 docker rm -f」。
        match self.docker(&crate::docker::rm_args(&name)).await {
            Ok(out) if out.status.success() => guard.mark_exited(),
            Ok(out) => tracing::warn!(
                target: "runner",
                "docker rm -f {name} 退出码 {:?};交给兜底守卫重试",
                out.status.code()
            ),
            Err(e) => tracing::warn!(
                target: "runner",
                "docker rm -f {name} 起不来:{e};交给兜底守卫重试"
            ),
        }

        let artifact = verdict(code, artifact)?;
        Ok(RunOutcome {
            exit_code: code,
            elapsed,
            artifact,
        })
    }
}

impl DockerRunner {
    /// `docker cp <容器>:<路径> -` 把产物作为 tar 流读回来,解出文件内容。
    ///
    /// tar 里可能有前导的 pax 头,也可能有多条记录;取**最后一个**普通文件
    /// 条目的内容 —— `docker cp` 对单文件产出的是一条记录,多取是为了容错
    /// 老版本 docker 附带的元数据条目。全程在内存里,宿主一个字节都不落。
    async fn fetch_artifact(
        &self,
        name: &str,
        spec: &crate::docker::RunSpec,
    ) -> Result<Vec<u8>, String> {
        let path = crate::docker::container_artifact_path(spec);
        let out = self
            .docker(&crate::docker::cp_args(name, &path))
            .await
            .map_err(|e| format!("docker cp 起不来:{e}"))?;
        if !out.status.success() {
            return Err(format!(
                "docker cp 退出码 {:?}:{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        extract_one(&out.stdout)
    }
}

/// 从一段 tar 字节里取出唯一的普通文件内容。
fn extract_one(tar: &[u8]) -> Result<Vec<u8>, String> {
    let mut ar = tar::Archive::new(std::io::Cursor::new(tar));
    let mut found: Option<Vec<u8>> = None;
    let entries = ar
        .entries()
        .map_err(|e| format!("产物不是合法 tar:{e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("tar 条目损坏:{e}"))?;
        // 目录 / 符号链接没有内容可取,跳过。
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("读 tar 条目失败:{e}"))?;
        found = Some(buf);
    }
    found.ok_or_else(|| "tar 里没有普通文件条目".to_string())
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
        assert_eq!(verdict(Some(0), Some(vec![7u8; 8])).unwrap(), vec![7u8; 8]);
    }

    #[test]
    fn nonzero_exit_is_failure_even_with_artifact() {
        // cargo 有时会先写产物再报错(后端自己 warn/err 也算非 0)。
        match verdict(Some(101), Some(vec![0u8; 100])) {
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
    fn empty_artifact_counts_as_missing() {
        // 空字节同样判失败:宁可报"没有产物",也别把 0 字节的下载链接
        // 当成一次成功编译交出去。
        match verdict(Some(0), Some(Vec::new())) {
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

    /// 造一个只含单个文件的 tar 字节,用来喂 [`extract_one`]。
    fn tar_of(name: &str, body: &[u8]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_size(body.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, name, body).unwrap();
        b.into_inner().unwrap()
    }

    #[test]
    fn extract_one_reads_the_single_file_out_of_the_tar() {
        // 这就是 `docker cp <path> -` 的产物形状:一个 tar,一条记录。
        let got = extract_one(&tar_of("rust2genshin_demo.gia", b"GIA!")).unwrap();
        assert_eq!(got, b"GIA!");
    }

    #[test]
    fn extract_one_skips_metadata_entries_and_takes_the_file() {
        // 老版本 docker 的 cp 会在文件前后附目录 / pax 记录;真实内容不能被它们挤掉。
        let mut b = tar::Builder::new(Vec::new());
        let mut d = tar::Header::new_gnu();
        d.set_entry_type(tar::EntryType::Directory);
        d.set_size(0);
        d.set_cksum();
        b.append(&d, std::io::empty()).unwrap();
        let mut f = tar::Header::new_gnu();
        f.set_size(4);
        f.set_mode(0o644);
        f.set_cksum();
        b.append_data(&mut f, "nested/rust2genshin_demo.gia", &b"GIA!"[..])
            .unwrap();
        let raw = b.into_inner().unwrap();
        assert_eq!(extract_one(&raw).unwrap(), b"GIA!");
    }

    #[test]
    fn extract_one_rejects_a_tar_without_a_file() {
        // 只有目录的 tar 要报"没有普通文件",而不是把空 Vec 当产物交出去。
        let mut b = tar::Builder::new(Vec::new());
        let mut d = tar::Header::new_gnu();
        d.set_entry_type(tar::EntryType::Directory);
        d.set_size(0);
        d.set_cksum();
        b.append(&d, std::io::empty()).unwrap();
        assert!(extract_one(&b.into_inner().unwrap()).is_err());
    }
}
