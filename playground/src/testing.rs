//! 只在测试里编译的替身。放一个文件而不是每个测试各写一份,
//! 是为了 HTTP / WS / 整条链路三处用的是同一个行为定义。

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    bake::{BuiltImage, ImageBuilder},
    docker::Stream,
    gc::DiskGate,
    job::JobPaths,
    runner::{JobRunner, JobSpec, LogSink, RunOutcome, RunnerError},
};

/// 假的 `ImageBuilder`:按顺序吐出预设结果。没结果了就报错,好让
/// "多烘了一次"这种 bug 现形而不是静默成功。
pub struct FakeBuilder {
    results: Mutex<VecDeque<Result<BuiltImage, String>>>,
    lines: Vec<String>,
    calls: Mutex<Vec<Option<String>>>,
}

impl FakeBuilder {
    pub fn new(results: Vec<Result<BuiltImage, String>>) -> Arc<Self> {
        Arc::new(Self {
            results: Mutex::new(results.into()),
            lines: vec!["step 1/4".into(), "step 4/4".into()],
            calls: Mutex::new(Vec::new()),
        })
    }

    pub fn calls(&self) -> Vec<Option<String>> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ImageBuilder for FakeBuilder {
    async fn build(
        &self,
        sha: Option<&str>,
        log: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<BuiltImage, String> {
        self.calls.lock().unwrap().push(sha.map(str::to_string));
        for l in &self.lines {
            log(l);
        }
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err("没有更多预设结果".into()))
    }
}

/// 水位永远够。`Arc<dyn DiskGate>` 的两个常用替身之一。
pub struct OkGate;

#[async_trait::async_trait]
impl DiskGate for OkGate {
    async fn ensure_free(&self) -> Result<(), String> {
        Ok(())
    }
}

/// 水位永远不够 —— 用来钉住 `no_disk` 那条路径不需要碰 docker。
pub struct LowGate;

#[async_trait::async_trait]
impl DiskGate for LowGate {
    async fn ensure_free(&self) -> Result<(), String> {
        Err("磁盘可用空间 1 字节低于水位".into())
    }
}

#[derive(Clone)]
pub enum Behavior {
    /// 吐这些行,退出码 0,写出产物。
    Ok { lines: Vec<(Stream, String)>, bytes: u64 },
    /// 吐这些行,退出码非 0,不写产物。
    Fail { lines: Vec<(Stream, String)>, code: i32 },
    /// 卡住不返回,让上层的超时路径被走到。
    Hang,
}

pub struct FakeRunner {
    behavior: Mutex<Behavior>,
    calls: Mutex<Vec<String>>,
    /// 每次 `run` 收到的 `JobSpec.target`。`calls` 只存 task,而目标
    /// 选择要单独钉住 —— 校验放行不等于真的传到了 Runner。
    targets: Mutex<Vec<String>>,
    running: AtomicUsize,
}

/// 让 `running` 在 abort 路径上也归还 —— `Behavior::Hang` 的 future 被
/// 上层丢掉时,函数体后半段根本不会执行。
struct RunningGuard<'a>(&'a AtomicUsize);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl FakeRunner {
    pub fn new(behavior: Behavior) -> Arc<Self> {
        Arc::new(Self {
            behavior: Mutex::new(behavior),
            calls: Mutex::new(Vec::new()),
            targets: Mutex::new(Vec::new()),
            running: AtomicUsize::new(0),
        })
    }

    /// 中途换行为。测试用它模拟「容器卡住 → 上层超时把它掐了」这类时序
    /// 变化,所以必须在 `run` 已经开始之后还能改 —— 字段是 `Mutex` 而不是
    /// 裸 `Behavior` 就是为了这一步。
    pub fn set(&self, b: Behavior) {
        *self.behavior.lock().unwrap() = b;
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn targets(&self) -> Vec<String> {
        self.targets.lock().unwrap().clone()
    }

    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl JobRunner for FakeRunner {
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError> {
        self.calls.lock().unwrap().push(job.task.clone());
        self.targets.lock().unwrap().push(job.target.clone());
        self.running.fetch_add(1, Ordering::SeqCst);
        // 计数借 Drop guard 归还:`Behavior::Hang` 的 future 被上层丢掉时,
        // 函数体后半段不会执行,只有 Drop 能在 abort 路径上把计数减回去。
        let _running = RunningGuard(&self.running);
        // 先克隆一份再 match:锁只在这一行里握着,后面 `run` 里要 await,
        // 拿着 `MutexGuard` 跨 await 会让整个 fake 变成 !Send。
        let behavior = self.behavior.lock().unwrap().clone();
        match &behavior {
            Behavior::Ok { lines, bytes } => {
                for (s, t) in lines {
                    log(*s, t);
                }
                std::fs::write(&job.paths.artifact, vec![0u8; *bytes as usize]).unwrap();
                Ok(RunOutcome {
                    exit_code: Some(0),
                    elapsed: Duration::from_millis(1),
                    artifact_bytes: *bytes,
                })
            }
            Behavior::Fail { lines, code } => {
                for (s, t) in lines {
                    log(*s, t);
                }
                Err(RunnerError::NoArtifact { code: Some(*code) })
            }
            Behavior::Hang => {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                unreachable!()
            }
        }
    }
}

/// 给测试用的 `JobPaths`:在临时目录里建一份真的。
pub fn fake_paths(dir: &std::path::Path, task: &str) -> JobPaths {
    let src = dir.join(task).join("src");
    let out = dir.join(task).join("out");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    JobPaths {
        artifact: out.join(crate::job::ARTIFACT_NAME),
        dir: dir.join(task),
        src_dir: src,
        out_dir: out,
    }
}

/// 在 `dir` 里跑一条 git 命令,断言成功,返回 stdout。
/// 造 git 环境用(init / commit / fetch / reset …)。要测的 git 操作本身
/// (比如 `Repo::pull_ff_only`)走 `Repo`,不走这里 —— 这个 helper 只是脚手架。
pub fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("跑 git 失败");
    assert!(
        out.status.success(),
        "git {args:?} 失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// 环境里有没有 git —— 打真 git 的测试(如 `pull_ff_only`)没 git 就跳过。
pub fn git_available() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 造一个"远端"仓库:init + 写 `f.txt` + commit,返回 (dir, head sha)。
/// 本地仓库 `remote add origin <remote>` 后即可 `fetch`。
pub fn git_remote_with_commit(content: &str) -> (tempfile::TempDir, String) {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "--initial-branch=main"]);
    std::fs::write(d.path().join("f.txt"), content).unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-m", "commit"]);
    let sha = git(d.path(), &["rev-parse", "HEAD"]);
    (d, sha)
}

/// 把一个本地路径转成 git 认的**本地**路径。Windows 下 `Path::to_str`
/// 给的是反斜杠(`C:\...`),git 会把 `\` 之后的 `C:` 段当成 scp 式
/// `host:path` 语法报 "hostname contains invalid characters";`file://` 又会
/// 被 git-for-windows 的路径转换 mangling 成 `/?/C:...`。正解是把分隔符
/// 换成正斜杠、不加 scheme —— `C:/Users/...` 直接被当成本地路径。
fn git_local_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 造一个"本地"仓库:init main + 把 `origin` 指向 `remote`,fetch 后
/// `reset --hard origin/main`(本地 HEAD = 远端当前 HEAD)。
pub fn git_local_clone(remote: &std::path::Path) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "--initial-branch=main"]);
    // 别 `canonicalize`:Windows 下它会给出 `\\?\` 前缀的 extended-length
    // 路径,换成正斜杠后 git 也认不得。`TempDir::path()` 本来就是绝对路径。
    git(d.path(), &["remote", "add", "origin", &git_local_path(remote)]);
    git(d.path(), &["fetch", "origin"]);
    git(d.path(), &["reset", "--hard", "origin/main"]);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(dir: &std::path::Path, task: &str) -> JobSpec {
        JobSpec {
            task: task.into(),
            source: "fn main() {}".into(),
            target: "beyond-server".into(),
            image_tag: "rust2genshin-play:0123456".into(),
            image_commit: "0123456".into(),
            paths: fake_paths(dir, task),
        }
    }

    #[tokio::test]
    async fn ok_writes_artifact_and_reports_bytes() {
        let d = tempfile::tempdir().unwrap();
        let r = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 7 });
        let s = spec(d.path(), "t1");
        let out = r.run(&s, Arc::new(|_, _| {})).await.unwrap();
        assert_eq!(out.artifact_bytes, 7);
        assert!(s.paths.artifact.is_file());
        assert_eq!(r.calls(), vec!["t1"]);
        assert_eq!(r.running(), 0, "收尾要把计数还回去");
    }

    #[tokio::test]
    async fn fail_leaves_no_artifact() {
        let d = tempfile::tempdir().unwrap();
        let r = FakeRunner::new(Behavior::Fail { lines: vec![], code: 101 });
        let s = spec(d.path(), "t1");
        assert!(r.run(&s, Arc::new(|_, _| {})).await.is_err());
        assert!(!s.paths.artifact.exists());
    }
}
