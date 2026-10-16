//! 只在测试里编译的替身。放一个文件而不是每个测试各写一份,
//! 是为了 HTTP / WS / 整条链路三处用的是同一个行为定义。

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    docker::Stream,
    job::JobPaths,
    runner::{JobRunner, JobSpec, LogSink, RunOutcome, RunnerError},
};

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

    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl JobRunner for FakeRunner {
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError> {
        self.calls.lock().unwrap().push(job.task.clone());
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
