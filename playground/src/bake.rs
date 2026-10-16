//! 烘焙状态机。
//!
//! ```text
//! Idle ──bake(sha X)──→ Baking(X) ──ok──→ Ready(X, serving)
//!                           │
//!                           └──fail──→ Ready(旧 serving) + baking.state = failed
//! ```
//!
//! 三条规矩:**单飞**(同一时刻只有一个烘焙,免得把机器打满)、
//! **折叠 pending**(烘焙中又来的 push 只记最后一个,中间的没人来得及用)、
//! **失败保旧**(main 上推了个编不过的 commit 不该把 playground 打死)。
//! 从未成功过时进程继续跑,`/healthz` 报 not_ready 带日志尾巴 —— 前端能
//! 把日志显示出来,比进程没了强。

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tokio::sync::Notify;

use crate::gc::DiskGate;

/// `/status` 给的日志尾巴行数。
pub const RING_CAPACITY: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BakeState {
    Running,
    Ok,
    Failed,
    NoDisk,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BakeRequest {
    /// `None` = 烘当前工作区 HEAD,由 `ImageBuilder` 自己解析。
    pub sha: Option<String>,
    /// 给 `/status` 看的一句话(pull 跳过的原因之类)。
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltImage {
    pub sha: String,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BakingStatus {
    pub sha: Option<String>,
    pub state: BakeState,
    pub note: Option<String>,
    pub tail: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BakeStatus {
    pub serving: Option<String>,
    pub serving_tag: Option<String>,
    pub baking: Option<BakingStatus>,
}

#[async_trait::async_trait]
pub trait ImageBuilder: Send + Sync {
    /// 生成镜像。`sha = None` 表示烘当前工作区 HEAD(实现自己 `git rev-parse`)。
    /// 日志逐行回调,由 `Baker` 收进 ring buffer。
    async fn build(
        &self,
        sha: Option<&str>,
        log: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<BuiltImage, String>;
}

/// 定长环形缓冲。烘焙输出逐行进这里(给 `/status` 看),同时原样打到 tty。
#[derive(Debug)]
pub struct Ring {
    cap: usize,
    lines: VecDeque<String>,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self { cap, lines: VecDeque::with_capacity(cap) }
    }

    pub fn push(&mut self, line: &str) {
        if self.lines.len() == self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line.to_string());
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

#[derive(Default)]
struct Inner {
    serving: Option<String>,
    serving_tag: Option<String>,
    /// 只能有一个:**覆盖**就是折叠。
    pending: Option<BakeRequest>,
    baking: Option<BakingStatus>,
}

pub struct Baker {
    builder: Arc<dyn ImageBuilder>,
    gate: Arc<dyn DiskGate>,
    inner: Mutex<Inner>,
    ring: Mutex<Ring>,
    kick: Notify,
}

impl Baker {
    pub fn new(builder: Arc<dyn ImageBuilder>, gate: Arc<dyn DiskGate>) -> Arc<Self> {
        Arc::new(Self {
            builder,
            gate,
            inner: Mutex::new(Inner::default()),
            ring: Mutex::new(Ring::new(RING_CAPACITY)),
            kick: Notify::new(),
        })
    }

    pub fn request(&self, req: BakeRequest) {
        self.inner.lock().expect("烘焙状态锁中毒").pending = Some(req);
        self.kick.notify_one();
    }

    pub fn status(&self) -> BakeStatus {
        let inner = self.inner.lock().expect("烘焙状态锁中毒");
        BakeStatus {
            serving: inner.serving.clone(),
            serving_tag: inner.serving_tag.clone(),
            baking: inner.baking.clone().map(|mut b| {
                b.tail = self.ring.lock().expect("烘焙日志锁中毒").snapshot();
                b
            }),
        }
    }

    pub fn tail(&self) -> Vec<String> {
        self.ring.lock().expect("烘焙日志锁中毒").snapshot()
    }

    pub fn ready(&self) -> bool {
        self.inner.lock().expect("烘焙状态锁中毒").serving.is_some()
    }

    /// 循环体的一步:有 pending 就烘一次,返回是否干了活。测试直接调它,
    /// 时序完全确定。
    pub async fn step(self: &Arc<Self>) -> bool {
        let Some(req) = self
            .inner
            .lock()
            .expect("烘焙状态锁中毒")
            .pending
            .take()
        else {
            return false;
        };

        {
            let mut inner = self.inner.lock().expect("烘焙状态锁中毒");
            inner.baking = Some(BakingStatus {
                sha: req.sha.clone(),
                state: BakeState::Running,
                note: req.note.clone(),
                tail: Vec::new(),
            });
        }
        self.ring.lock().expect("烘焙日志锁中毒").clear();

        let me = Arc::clone(self);
        let log = move |line: &str| {
            tracing::info!(target: "bake", "{line}");
            me.ring.lock().expect("烘焙日志锁中毒").push(line);
        };

        let outcome = match self.gate.ensure_free().await {
            Err(why) => Err((BakeState::NoDisk, why)),
            Ok(()) => match self.builder.build(req.sha.as_deref(), &log).await {
                Ok(built) => {
                    let mut inner = self.inner.lock().expect("烘焙状态锁中毒");
                    inner.serving = Some(built.sha);
                    inner.serving_tag = Some(built.tag);
                    Ok(())
                }
                Err(why) => Err((BakeState::Failed, why)),
            },
        };

        if let Err((state, note)) = outcome {
            // 失败保旧:serving 一个字都不动。
            let mut inner = self.inner.lock().expect("烘焙状态锁中毒");
            inner.baking = Some(BakingStatus {
                sha: req.sha,
                state,
                note: Some(note),
                tail: Vec::new(),
            });
        } else {
            self.inner.lock().expect("烘焙状态锁中毒").baking = None;
        }
        true
    }

    /// 后台循环:没活干就在 `kick` 上等,不轮询。
    pub async fn run_loop(self: Arc<Self>) {
        loop {
            if !self.step().await {
                self.kick.notified().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
        time::Duration,
    };

    use super::*;

    struct FakeBuilder {
        results: Mutex<VecDeque<Result<BuiltImage, String>>>,
        lines: Vec<String>,
        calls: Mutex<Vec<Option<String>>>,
    }

    impl FakeBuilder {
        fn new(results: Vec<Result<BuiltImage, String>>) -> Arc<Self> {
            Arc::new(Self {
                results: Mutex::new(results.into()),
                lines: vec!["step 1/4".into(), "step 4/4".into()],
                calls: Mutex::new(Vec::new()),
            })
        }
        fn calls(&self) -> Vec<Option<String>> {
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

    struct OkGate;
    #[async_trait::async_trait]
    impl DiskGate for OkGate {
        async fn ensure_free(&self) -> Result<(), String> {
            Ok(())
        }
    }

    struct LowGate;
    #[async_trait::async_trait]
    impl DiskGate for LowGate {
        async fn ensure_free(&self) -> Result<(), String> {
            Err("磁盘可用空间 1 字节低于水位".into())
        }
    }

    fn image(sha: &str) -> BuiltImage {
        BuiltImage { sha: sha.into(), tag: format!("rust2genshin-play:{sha}") }
    }

    fn baker(b: Arc<FakeBuilder>, g: Arc<dyn DiskGate>) -> Arc<Baker> {
        Baker::new(b, g)
    }

    #[tokio::test]
    async fn first_bake_sets_serving() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        assert!(!k.ready());
        k.request(BakeRequest::default());
        assert!(k.step().await);
        let s = k.status();
        assert!(k.ready());
        assert_eq!(s.serving.as_deref(), Some("aaa1111"));
        assert_eq!(s.serving_tag.as_deref(), Some("rust2genshin-play:aaa1111"));
        assert_eq!(s.baking, None, "成功后不再报 baking");
    }

    #[tokio::test]
    async fn failure_keeps_the_old_serving_image() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111")), Err("编译不过".into())]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        k.request(BakeRequest::default());
        k.step().await;
        k.request(BakeRequest { sha: Some("bbb2222".into()), note: None });
        k.step().await;
        let s = k.status();
        assert_eq!(s.serving.as_deref(), Some("aaa1111"), "失败的烘焙不该换掉 serving");
        let baking = s.baking.expect("失败要留在状态里");
        assert_eq!(baking.state, BakeState::Failed);
        assert_eq!(baking.sha.as_deref(), Some("bbb2222"));
        assert_eq!(baking.note.as_deref(), Some("编译不过"));
    }

    #[tokio::test]
    async fn no_disk_skips_the_builder() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(LowGate));
        k.request(BakeRequest::default());
        assert!(k.step().await);
        assert!(b.calls().is_empty(), "水位不够时不该起 docker build");
        let s = k.status();
        assert!(!k.ready());
        let baking = s.baking.unwrap();
        assert_eq!(baking.state, BakeState::NoDisk);
        assert!(baking.note.unwrap().contains("磁盘"));
    }

    #[tokio::test]
    async fn pending_is_folded_to_the_latest() {
        // 三个 request 折叠成一次 build,所以这里只该有一个预设结果
        // (FakeBuilder 是 pop_front 取结果,多放的那个永远轮不到)。
        let b = FakeBuilder::new(vec![Ok(image("ccc3333"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        k.request(BakeRequest { sha: Some("aaa1111".into()), note: None });
        k.request(BakeRequest { sha: Some("bbb2222".into()), note: None });
        k.request(BakeRequest { sha: Some("ccc3333".into()), note: None });
        assert!(k.step().await);
        assert!(!k.step().await, "折叠之后只剩一次烘焙");
        assert_eq!(b.calls(), vec![Some("ccc3333".into())]);
        assert_eq!(k.status().serving.as_deref(), Some("ccc3333"));
    }

    #[tokio::test]
    async fn log_lines_land_in_the_tail() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        k.request(BakeRequest::default());
        k.step().await;
        assert_eq!(k.tail(), vec!["step 1/4".to_string(), "step 4/4".to_string()]);
    }

    #[tokio::test]
    async fn tail_is_capped() {
        let mut r = Ring::new(3);
        for i in 0..10 {
            r.push(&format!("line {i}"));
        }
        assert_eq!(r.snapshot(), vec!["line 7", "line 8", "line 9"]);
    }

    #[tokio::test]
    async fn run_loop_bakes_after_a_request() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        let loop_handle = {
            let k = Arc::clone(&k);
            tokio::spawn(async move { k.run_loop().await })
        };
        // 循环已经在等 kick 了再发请求,免得测试靠 sleep 猜时序。
        tokio::time::sleep(Duration::from_millis(20)).await;
        k.request(BakeRequest::default());
        for _ in 0..50 {
            if k.ready() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(k.ready(), "run_loop 必须把请求烘出来");
        loop_handle.abort();
    }

    #[tokio::test]
    async fn step_without_a_request_does_nothing() {
        let b = FakeBuilder::new(vec![]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        assert!(!k.step().await);
        assert!(b.calls().is_empty());
    }
}
